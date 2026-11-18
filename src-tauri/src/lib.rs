mod audio;
mod config;
mod draft;
mod engine;
mod hotkeys;
mod models;
mod overlay;
mod sound;
mod stt;
mod text;
mod tray;

use config::{Config, Settings};
use engine::{Command, Engine, Host, Snapshot, Status};
use hotkeys::HotkeyStatus;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

/// App-wide state, shared by the engine host, the UI commands, tray and hotkeys.
struct Shared {
    latest: Mutex<Option<Snapshot>>,
    settings: Mutex<Settings>,
    hotkeys: Mutex<HotkeyStatus>,
    /// Set once at startup. Not behind a Mutex on purpose: tray calls block
    /// until the main thread runs them, so holding a lock across one can
    /// deadlock with the main thread. Only touch it on the main thread.
    tray: OnceLock<tray::Tray>,
    overlay_shown: AtomicBool,
    downloading: AtomicBool,
    cancel_download: AtomicBool,
}

/// Connects the engine to the app.
struct TauriHost {
    app: AppHandle,
}

impl Host for TauriHost {
    fn update(&self, snapshot: &Snapshot) {
        let shared = self.app.state::<Shared>();
        // Stored without the one-off message, so a page that loads later
        // doesn't replay an old "Copied 120 words".
        *shared.latest.lock().unwrap() = Some(Snapshot { msg: String::new(), ..snapshot.clone() });
        let (app, snap) = (self.app.clone(), snapshot.clone());
        // Called on the engine thread; don't make it wait for the main thread.
        let _ = self.app.run_on_main_thread(move || {
            if let Some(tray) = app.state::<Shared>().tray.get() {
                tray.update(&snap);
            }
        });
        let _ = self.app.emit("state", snapshot);
        refresh_overlay(&self.app);
    }

    fn copy_to_clipboard(&self, text: &str) -> anyhow::Result<()> {
        Ok(self.app.clipboard().write_text(text)?)
    }

    fn chime(&self, chime: sound::Chime) {
        if self.app.state::<Shared>().settings.lock().unwrap().chimes {
            sound::play(chime);
        }
    }
}

/// The overlay is up while dictation is on, unless the user hid it.
fn refresh_overlay(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let on = shared.latest.lock().unwrap().as_ref().is_some_and(|s| s.status != Status::Off);
    let show = on && shared.settings.lock().unwrap().overlay_enabled;
    if shared.overlay_shown.swap(show, Ordering::SeqCst) != show {
        overlay::set_visible(app, show);
    }
}

pub(crate) fn toggle_overlay(app: &AppHandle) {
    let mut settings = app.state::<Shared>().settings.lock().unwrap().clone();
    settings.overlay_enabled = !settings.overlay_enabled;
    let _ = settings.save(&Settings::path());
    apply_settings(app, settings);
}

pub(crate) fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub(crate) fn open_draft_folder(app: &AppHandle) {
    let dir = Config::from_settings(&app.state::<Shared>().settings.lock().unwrap()).draft_dir;
    let _ = std::fs::create_dir_all(&dir);
    let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
}

/// Makes `settings` current: engine, hotkeys, overlay, tray, autostart.
fn apply_settings(app: &AppHandle, settings: Settings) {
    let shared = app.state::<Shared>();
    if let Some(engine) = app.try_state::<Engine>() {
        engine.configure(Config::from_settings(&settings));
    }
    let hotkey_status = hotkeys::apply(app, &settings);
    *shared.hotkeys.lock().unwrap() = hotkey_status;
    overlay::set_position(app, settings.overlay_position);
    let (handle, checked) = (app.clone(), settings.overlay_enabled);
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = handle.state::<Shared>().tray.get() {
            tray.set_overlay_checked(checked);
        }
    });
    let autostart = app.autolaunch();
    if autostart.is_enabled().unwrap_or(false) != settings.start_on_login {
        let _ = if settings.start_on_login { autostart.enable() } else { autostart.disable() };
    }
    let _ = app.emit("settings", &settings);
    *shared.settings.lock().unwrap() = settings;
    refresh_overlay(app);
}

// ---- Commands for the UI ---------------------------------------------------

/// Lets a page that loads late (the overlay) fetch the current state.
#[tauri::command]
fn get_state(shared: tauri::State<'_, Shared>) -> Option<Snapshot> {
    shared.latest.lock().unwrap().clone()
}

#[tauri::command]
fn get_settings(shared: tauri::State<'_, Shared>) -> Settings {
    shared.settings.lock().unwrap().clone()
}

/// Saves and applies settings; returns how hotkey registration went.
#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<HotkeyStatus, String> {
    settings.save(&Settings::path()).map_err(|e| format!("Couldn't save settings: {e}"))?;
    apply_settings(&app, settings);
    Ok(app.state::<Shared>().hotkeys.lock().unwrap().clone())
}

#[tauri::command]
fn get_hotkey_status(shared: tauri::State<'_, Shared>) -> HotkeyStatus {
    shared.hotkeys.lock().unwrap().clone()
}

#[tauri::command]
fn send_command(app: AppHandle, command: String) -> Result<(), String> {
    run_command(&app, &command).then_some(()).ok_or_else(|| format!("Unknown command {command:?}"))
}

#[tauri::command]
fn open_folder(app: AppHandle) {
    open_draft_folder(&app);
}

/// Facts about this machine the settings window shows.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Environment {
    wayland: bool,
    /// Path to this binary, for compositor keybind lines.
    exe: PathBuf,
    settings_file: PathBuf,
    draft_dir: PathBuf,
    model: PathBuf,
    model_found: bool,
    default_draft_dir: PathBuf,
    default_model: PathBuf,
}

#[tauri::command]
fn environment(shared: tauri::State<'_, Shared>) -> Environment {
    let config = Config::from_settings(&shared.settings.lock().unwrap());
    Environment {
        wayland: hotkeys::is_wayland(),
        exe: std::env::current_exe().unwrap_or_else(|_| "dictator".into()),
        settings_file: Settings::path(),
        model_found: config.model.is_file(),
        draft_dir: config.draft_dir,
        model: config.model,
        default_draft_dir: Config::default_draft_dir(),
        default_model: Config::default_model(),
    }
}

/// A model the settings window can offer.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelEntry {
    id: &'static str,
    label: &'static str,
    note: &'static str,
    bytes: u64,
    downloaded: bool,
    in_use: bool,
}

#[tauri::command]
fn list_models(shared: tauri::State<'_, Shared>) -> Vec<ModelEntry> {
    let current = Config::from_settings(&shared.settings.lock().unwrap()).model;
    models::MODELS
        .iter()
        .map(|m| ModelEntry {
            id: m.id,
            label: m.label,
            note: m.note,
            bytes: m.bytes,
            downloaded: m.path().is_file(),
            in_use: m.path() == current,
        })
        .collect()
}

/// Downloads a model (one at a time), emitting `model-download` progress.
#[tauri::command]
async fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    let model = models::find(&id).ok_or("Unknown model")?;
    let shared = app.state::<Shared>();
    if shared.downloading.swap(true, Ordering::SeqCst) {
        return Err("Another download is already running".into());
    }
    shared.cancel_download.store(false, Ordering::SeqCst);
    let result = models::download(model, &shared.cancel_download, |p| {
        let _ = app.emit("model-download", p);
    })
    .await;
    shared.downloading.store(false, Ordering::SeqCst);
    result.map(|_| ()).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn cancel_download(shared: tauri::State<'_, Shared>) {
    shared.cancel_download.store(true, Ordering::SeqCst);
}

/// Switches to a downloaded model. Takes effect from the next "on".
#[tauri::command]
fn use_model(app: AppHandle, id: String) -> Result<(), String> {
    let model = models::find(&id).ok_or("Unknown model")?;
    let path = model.path();
    if !path.is_file() {
        return Err("Download it first".into());
    }
    let mut settings = app.state::<Shared>().settings.lock().unwrap().clone();
    // The default model is stored as "none" so a future default can replace it.
    settings.model = (path != Config::default_model()).then_some(path);
    settings.save(&Settings::path()).map_err(|e| format!("Couldn't save settings: {e}"))?;
    apply_settings(&app, settings);
    Ok(())
}

// ---- CLI ---------------------------------------------------------------------

/// Runs a command by name: engine commands plus app-level ones.
/// Returns false if the name is unknown.
fn run_command(app: &AppHandle, name: &str) -> bool {
    match name {
        "overlay" => toggle_overlay(app),
        "settings" => show_settings(app),
        "quit" => app.exit(0),
        _ => match (Command::parse(name), app.try_state::<Engine>()) {
            (Some(command), Some(engine)) => engine.send(command),
            (Some(_), None) => {} // still starting up
            (None, _) => return false,
        },
    }
    true
}

/// `dictator <command>`, from this process's own arguments or a second launch.
fn run_cli(app: &AppHandle, args: &[String]) {
    if let Some(arg) = args.get(1) {
        if !run_command(app, arg) {
            eprintln!(
                "dictator: unknown command {arg:?}. Expected one of: {}, overlay, settings, quit",
                Command::NAMES
            );
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings_path = Settings::path();
    let first_run = !settings_path.exists();
    let settings = Settings::load(&settings_path);

    tauri::Builder::default()
        // Must be first: a second `dictator <command>` hands its arguments to the
        // running instance here and exits before creating any windows.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| run_cli(app, &args)))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .manage(Shared {
            latest: Mutex::new(None),
            settings: Mutex::new(settings.clone()),
            hotkeys: Mutex::new(HotkeyStatus::default()),
            tray: OnceLock::new(),
            overlay_shown: AtomicBool::new(false),
            downloading: AtomicBool::new(false),
            cancel_download: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_settings,
            save_settings,
            get_hotkey_status,
            send_command,
            open_folder,
            environment,
            list_models,
            download_model,
            cancel_download,
            use_model,
        ])
        .on_window_event(|window, event| {
            // Closing the settings window hides it; the app lives in the tray.
            if let (WindowEvent::CloseRequested { api, .. }, "main") = (event, window.label()) {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            let handle = app.handle();
            overlay::create(handle)?;
            let _ = app.state::<Shared>().tray.set(tray::create(handle, settings.overlay_enabled)?);
            let engine = Engine::spawn(Config::from_settings(&settings), TauriHost { app: handle.clone() })?;
            app.manage(engine);
            apply_settings(handle, settings.clone());

            // Show settings on first run, or if the model is missing, so a new
            // user isn't left wondering why nothing happens.
            if first_run || !Config::from_settings(&settings).model.is_file() {
                show_settings(handle);
            }
            run_cli(handle, &std::env::args().collect::<Vec<_>>());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
