//! Tray icon and menu. The tray is the app's home: the settings window is
//! opened from here, and closing it just hides it.

use crate::engine::{Command, Engine, Snapshot, Status};
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

/// Menu items whose text changes with the state, and the icon.
pub struct Tray {
    power: MenuItem<Wry>,
    record: MenuItem<Wry>,
    overlay: CheckMenuItem<Wry>,
    icon: tauri::tray::TrayIcon<Wry>,
    icons: Icons,
    shown: Mutex<Option<Status>>,
}

/// The tray icon in each state, so you can tell what's going on even with the
/// overlay hidden: greyed out when off, a red dot while listening, a yellow
/// dot while busy.
struct Icons {
    off: Image<'static>,
    ready: Image<'static>,
    recording: Image<'static>,
    busy: Image<'static>,
}

impl Icons {
    fn new() -> Self {
        let base = Image::from_bytes(include_bytes!("../icons/64x64.png")).expect("bundled tray icon");
        Self {
            off: dimmed(&base),
            ready: base.clone(),
            recording: badge(&base, [0xe5, 0x48, 0x4d]),
            busy: badge(&base, [0xf5, 0xc5, 0x42]),
        }
    }

    fn get(&self, status: Status) -> &Image<'static> {
        match status {
            Status::Off => &self.off,
            Status::Ready => &self.ready,
            Status::Recording => &self.recording,
            Status::Loading | Status::Transcribing => &self.busy,
        }
    }
}

/// Greyscale at reduced opacity.
fn dimmed(base: &Image<'_>) -> Image<'static> {
    let mut rgba = base.rgba().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        let grey = ((u32::from(px[0]) * 3 + u32::from(px[1]) * 6 + u32::from(px[2])) / 10) as u8;
        px[..3].fill(grey);
        px[3] = (u32::from(px[3]) * 55 / 100) as u8;
    }
    Image::new_owned(rgba, base.width(), base.height())
}

/// A coloured dot with a dark ring in the bottom-right corner.
fn badge(base: &Image<'_>, color: [u8; 3]) -> Image<'static> {
    let (w, h) = (base.width(), base.height());
    let mut rgba = base.rgba().to_vec();
    let r = w as f32 * 0.24;
    let (cx, cy) = (w as f32 - r - 1.0, h as f32 - r - 1.0);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let px = &mut rgba[((y * w + x) * 4) as usize..][..4];
            if d <= r - 3.0 {
                px.copy_from_slice(&[color[0], color[1], color[2], 255]);
            } else if d <= r {
                px.copy_from_slice(&[0x15, 0x15, 0x1a, 255]);
            }
        }
    }
    Image::new_owned(rgba, w, h)
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

    let icons = Icons::new();
    let icon = TrayIconBuilder::with_id("main")
        .icon(icons.off.clone())
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

    Ok(Tray { power, record, overlay, icon, icons, shown: Mutex::new(Some(Status::Off)) })
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

        let mut shown = self.shown.lock().unwrap();
        if *shown != Some(snapshot.status) {
            *shown = Some(snapshot.status);
            let _ = self.icon.set_icon(Some(self.icons.get(snapshot.status).clone()));
        }
    }

    pub fn set_overlay_checked(&self, checked: bool) {
        let _ = self.overlay.set_checked(checked);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dumps the four icons as raw RGBA for eyeballing:
    ///   cargo test tray_icons -- --ignored
    ///   ffmpeg -f rawvideo -pix_fmt rgba -s 64x64 -i /tmp/tray-ready.rgba ready.png
    #[test]
    #[ignore]
    fn tray_icons() {
        let icons = Icons::new();
        for (name, img) in [
            ("off", &icons.off),
            ("ready", &icons.ready),
            ("recording", &icons.recording),
            ("busy", &icons.busy),
        ] {
            std::fs::write(format!("/tmp/tray-{name}.rgba"), img.rgba()).unwrap();
        }
    }
}
