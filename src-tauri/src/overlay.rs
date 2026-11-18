//! The overlay: a small transparent window that shows the draft above everything,
//! including fullscreen games. It never takes focus and clicks pass through it.
//!
//! Each platform needs its own trick to stay on top:
//! - Wayland: wlr-layer-shell "overlay" layer (Hyprland, Sway, KDE; not GNOME).
//! - Windows / X11 / macOS: a regular always-on-top window.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 780.0;
const HEIGHT: f64 = 180.0;
const TOP_MARGIN: i32 = 10;

pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay".into()))
        .title("Dictator overlay")
        .inner_size(WIDTH, HEIGHT)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;

    if !platform::setup(&window)? {
        window.show()?;
        // Click-through. Must come after show(): on GTK this sets an empty
        // input region on the realized window.
        window.set_ignore_cursor_events(true)?;
    }
    Ok(window)
}

pub fn toggle(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window(LABEL) {
        Some(w) if w.is_visible()? => w.hide(),
        Some(w) => {
            w.show()?;
            w.set_ignore_cursor_events(true)
        }
        None => create(app).map(|_| ()),
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use gtk::prelude::*;
    use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
    use tauri::WebviewWindow;

    /// Returns true if the overlay now lives in its own layer-shell surface.
    pub fn setup(window: &WebviewWindow) -> tauri::Result<bool> {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        if !wayland
            || !gtk_layer_shell::is_supported()
            || std::env::var_os("DICTATOR_NO_LAYER_SHELL").is_some()
        {
            // X11, or a Wayland compositor without layer-shell (GNOME): fall back
            // to the plain always-on-top window from the builder.
            return Ok(false);
        }
        // Layer-shell must be set up before a window is first mapped, but tao
        // realizes its windows on creation. So move the webview into a fresh
        // GTK window that we configure first; tao's own window stays hidden.
        let tao_window = window.gtk_window()?;
        let vbox = window.default_vbox()?;
        tao_window.remove(&vbox);

        let layer = gtk::Window::new(gtk::WindowType::Toplevel);
        layer.set_app_paintable(true);
        if let Some(visual) = GtkWindowExt::screen(&layer).and_then(|s| s.rgba_visual()) {
            layer.set_visual(Some(&visual)); // per-pixel transparency
        }
        layer.init_layer_shell();
        layer.set_namespace("dictator-overlay");
        layer.set_layer(Layer::Overlay);
        layer.set_keyboard_mode(KeyboardMode::None);
        layer.set_exclusive_zone(-1); // don't push other windows around
        layer.set_anchor(Edge::Top, true);
        layer.set_layer_shell_margin(Edge::Top, super::TOP_MARGIN);
        layer.set_size_request(super::WIDTH as i32, super::HEIGHT as i32);
        layer.add(&vbox);
        layer.connect_realize(|w| {
            // Empty input region: every click goes to whatever is underneath.
            if let Some(gdk_window) = w.window() {
                gdk_window.input_shape_combine_region(&gtk::cairo::Region::create(), 0, 0);
            }
        });
        layer.show_all();
        Ok(true)
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use tauri::WebviewWindow;

    pub fn setup(_window: &WebviewWindow) -> tauri::Result<bool> {
        // TODO(windows): WS_EX_NOACTIVATE so it can't steal focus from the game.
        // TODO(macos): NSPanel level above fullscreen spaces.
        Ok(false)
    }
}
