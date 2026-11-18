//! Tray icon and menu. The tray is the app's home: the settings window is
//! opened from here, and closing it just hides it.

use crate::engine::{Command, Engine, Snapshot, Status};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

/// Menu items whose text changes with the state.
pub struct Tray {
    power: MenuItem<Wry>,
    record: MenuItem<Wry>,
    overlay: CheckMenuItem<Wry>,
    icon: tauri::tray::TrayIcon<Wry>,
}

pub fn create(app: &AppHandle, overlay_enabled: bool) -> tauri::Result<Tray> {
    let power = MenuItem::with_id(app, "power", "Turn on", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Start recording", false, None::<&str>)?;
    let copy = MenuItem::with_id(app, "copy", "Copy draft and clear", true, None::<&str>)?;
    let undo = MenuItem::with_id(app, "undo", "Undo last chunk", true, None::<&str>)?;
    let overlay =
        CheckMenuItem::with_id(app, "overlay", "Show overlay", true, overlay_enabled, None::<&str>)?;
    let folder = MenuItem::with_id(app, "folder", "Open draft folder", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Dictator", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(
        app,
        &[&power, &record, &sep()?, &copy, &undo, &overlay, &folder, &sep()?, &settings, &quit],
    )?;

    let icon = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundled icon"))
        .tooltip("Dictator: off")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let engine = app.state::<Engine>();
            match event.id().as_ref() {
                "power" => engine.send(Command::Power),
                "record" => engine.send(Command::Toggle),
                "copy" => engine.send(Command::Copy),
                "undo" => engine.send(Command::Undo),
                "overlay" => crate::toggle_overlay(app),
                "folder" => crate::open_draft_folder(app),
                "settings" => crate::show_settings(app),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;

    Ok(Tray { power, record, overlay, icon })
}

impl Tray {
    pub fn update(&self, snapshot: &Snapshot) {
        let on = snapshot.status != Status::Off;
        let _ = self.power.set_text(if on { "Turn off" } else { "Turn on" });
        let _ = self.record.set_enabled(matches!(
            snapshot.status,
            Status::Ready | Status::Recording | Status::Transcribing
        ));
        let _ = self.record.set_text(if snapshot.status == Status::Recording {
            "Stop recording"
        } else {
            "Start recording"
        });
        let label = match snapshot.status {
            Status::Off => "off",
            Status::Loading => "starting up",
            Status::Ready => "ready",
            Status::Recording => "listening",
            Status::Transcribing => "transcribing",
        };
        let _ = self.icon.set_tooltip(Some(format!("Dictator: {label}")));
    }

    pub fn set_overlay_checked(&self, checked: bool) {
        let _ = self.overlay.set_checked(checked);
    }
}
