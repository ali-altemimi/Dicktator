//! Built-in global hotkeys (Windows, macOS, X11).
//!
//! On Wayland, apps can't grab keys globally. There, users bind
//! `dictator <command>` in their compositor instead (the settings window
//! shows the lines to copy). The XDG GlobalShortcuts portal would be the
//! native route; that's planned.

use crate::config::{HotkeyMode, Settings};
use crate::engine::{Command, Engine};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    /// False on Wayland: bind the CLI commands in your compositor instead.
    pub native: bool,
    /// Hotkeys that couldn't be registered (taken by another app, bad syntax).
    pub errors: Vec<String>,
}

pub fn is_wayland() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// What each hotkey does. `overlay` isn't an engine command, so it's separate.
enum Action {
    Record,
    Engine(Command),
    Overlay,
}

/// (Re)registers all hotkeys from `settings`.
pub fn apply(app: &AppHandle, settings: &Settings) -> HotkeyStatus {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    if is_wayland() {
        return HotkeyStatus { native: false, errors: Vec::new() };
    }

    let keys = &settings.hotkeys;
    let hold = settings.hotkey_mode == HotkeyMode::Hold;
    let bindings = [
        ("Record", &keys.record, Action::Record),
        ("On/off", &keys.power, Action::Engine(Command::Power)),
        ("Copy", &keys.copy, Action::Engine(Command::Copy)),
        ("Undo", &keys.undo, Action::Engine(Command::Undo)),
        ("Overlay", &keys.overlay, Action::Overlay),
    ];
    let mut errors = Vec::new();
    for (name, key, action) in bindings {
        if key.trim().is_empty() {
            continue;
        }
        let result = shortcuts.on_shortcut(key.as_str(), move |app, _, event| {
            let pressed = event.state() == ShortcutState::Pressed;
            let command = match (&action, hold, pressed) {
                (Action::Record, false, true) => Some(Command::Toggle),
                (Action::Record, true, true) => Some(Command::Start),
                (Action::Record, true, false) => Some(Command::Stop),
                (Action::Engine(c), _, true) => Some(*c),
                (Action::Overlay, _, true) => {
                    crate::toggle_overlay(app);
                    None
                }
                _ => None,
            };
            if let Some(c) = command {
                app.state::<Engine>().send(c);
            }
        });
        if let Err(e) = result {
            errors.push(format!("{name} ({key}): {e}"));
        }
    }
    HotkeyStatus { native: true, errors }
}
