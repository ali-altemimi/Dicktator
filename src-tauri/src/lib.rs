mod audio;
mod config;
mod draft;
mod engine;
mod overlay;
mod stt;
mod text;

use engine::{Command, Engine, Host, Snapshot, Status};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Connects the engine to the app: pushes state to the overlay, owns the clipboard.
struct TauriHost {
    app: AppHandle,
    latest: Arc<Mutex<Option<Snapshot>>>,
}

impl Host for TauriHost {
    fn update(&self, snapshot: &Snapshot) {
        let mut latest = self.latest.lock().unwrap();
        let was_off = latest.as_ref().is_none_or(|s| s.status == Status::Off);
        let is_off = snapshot.status == Status::Off;
        // Stored without the one-off message, so a page that loads later
        // doesn't replay an old "Copied 120 words".
        *latest = Some(Snapshot { msg: String::new(), ..snapshot.clone() });
        drop(latest);

        let _ = self.app.emit_to(overlay::LABEL, "state", snapshot);
        // The overlay is only up while dictation is on.
        if was_off != is_off {
            overlay::set_visible(&self.app, !is_off);
        }
    }

    fn copy_to_clipboard(&self, text: &str) -> anyhow::Result<()> {
        Ok(self.app.clipboard().write_text(text)?)
    }
}

/// Lets a page that loads late (the overlay) fetch the current state.
#[tauri::command]
fn get_state(latest: tauri::State<'_, Arc<Mutex<Option<Snapshot>>>>) -> Option<Snapshot> {
    latest.lock().unwrap().clone()
}

/// `dictator toggle` etc. Returns true if `args` held a command.
fn run_cli(engine: &Engine, args: &[String]) -> bool {
    let Some(arg) = args.get(1) else { return false };
    match Command::parse(arg) {
        Some(command) => engine.send(command),
        None => eprintln!("dictator: unknown command {arg:?} (expected one of: {})", Command::NAMES),
    }
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second `dictator <command>` hands its arguments to the
        // running instance here and exits before creating any windows.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            run_cli(&app.state::<Engine>(), &args);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_state])
        .setup(|app| {
            overlay::create(app.handle())?;

            let latest = Arc::new(Mutex::new(None));
            app.manage(latest.clone());
            let host = TauriHost { app: app.handle().clone(), latest };
            let engine = Engine::spawn(config::Config::load(), host)?;
            run_cli(&engine, &std::env::args().collect::<Vec<_>>());
            app.manage(engine);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
