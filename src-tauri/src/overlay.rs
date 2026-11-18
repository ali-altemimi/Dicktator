//! The overlay: a small transparent window that shows the draft above everything,
//! including fullscreen games. It never takes focus and clicks pass through it.
//!
//! Each platform needs its own trick to stay on top:
//! - Wayland: wlr-layer-shell "overlay" layer (Hyprland, Sway, KDE; not GNOME).
//! - Windows: a topmost window that can't be activated (WS_EX_NOACTIVATE) and
//!   is shown with SW_SHOWNOACTIVATE, so it never takes focus from the game.
//! - X11 / macOS: a regular always-on-top window.

use crate::config::OverlayPosition;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 780.0;
const HEIGHT: f64 = 180.0;
const MARGIN: i32 = 10;

/// Creates the overlay, hidden. Call [`set_visible`] to show it.
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
        .focusable(false) // Windows: WS_EX_NOACTIVATE, never takes focus
        .visible(false)
        .build()?;
    platform::setup(&window)?;
    Ok(window)
}

/// Moves the overlay to the top or bottom centre of the screen. Any thread.
pub fn set_position(app: &AppHandle, position: OverlayPosition) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let _ = app.run_on_main_thread(move || {
        if platform::set_position(position) {
            return;
        }
        let monitor = window.current_monitor().ok().flatten();
        let Some(monitor) = monitor.or_else(|| window.primary_monitor().ok().flatten()) else {
            return;
        };
        let (origin, size, scale) = (monitor.position(), monitor.size(), monitor.scale_factor());
        let (w, h) = ((WIDTH * scale) as i32, (HEIGHT * scale) as i32);
        let margin = (f64::from(MARGIN) * scale) as i32;
        let x = origin.x + (size.width as i32 - w) / 2;
        let y = match position {
            OverlayPosition::Top => origin.y + margin,
            OverlayPosition::Bottom => origin.y + size.height as i32 - h - margin,
        };
        let _ = window.set_position(PhysicalPosition::new(x, y));
    });
}

/// Shows or hides the overlay. Safe to call from any thread.
pub fn set_visible(app: &AppHandle, visible: bool) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let _ = app.run_on_main_thread(move || {
        if platform::set_visible(visible) {
            return;
        }
        if visible {
            let _ = window.show();
            // Click-through. Must come after show(): on GTK this sets an
            // empty input region on the realized window.
            let _ = window.set_ignore_cursor_events(true);
        } else {
            let _ = window.hide();
        }
    });
}

#[cfg(target_os = "linux")]
mod platform {
    use crate::config::OverlayPosition;
    use gtk::prelude::*;
    use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
    use std::cell::RefCell;
    use tauri::WebviewWindow;

    thread_local! {
        // The layer-shell window, if we made one. GTK objects live on the main thread.
        static LAYER: RefCell<Option<gtk::Window>> = const { RefCell::new(None) };
    }

    /// Shows/hides the layer-shell window. Returns false if there isn't one
    /// (the caller then uses the normal Tauri window). Main thread only.
    pub fn set_visible(visible: bool) -> bool {
        LAYER.with_borrow(|layer| match layer {
            Some(w) if visible => {
                w.show();
                true
            }
            Some(w) => {
                w.hide();
                true
            }
            None => false,
        })
    }

    /// Re-anchors the layer-shell window. Returns false if there isn't one.
    pub fn set_position(position: OverlayPosition) -> bool {
        LAYER.with_borrow(|layer| {
            let Some(w) = layer else { return false };
            let top = position == OverlayPosition::Top;
            w.set_anchor(Edge::Top, top);
            w.set_anchor(Edge::Bottom, !top);
            w.set_layer_shell_margin(if top { Edge::Top } else { Edge::Bottom }, super::MARGIN);
            true
        })
    }

    pub fn setup(window: &WebviewWindow) -> tauri::Result<()> {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        if !wayland
            || !gtk_layer_shell::is_supported()
            || std::env::var_os("DICTATOR_NO_LAYER_SHELL").is_some()
        {
            // X11, or a Wayland compositor without layer-shell (GNOME): fall back
            // to the plain always-on-top window from the builder.
            return Ok(());
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
        layer.set_layer_shell_margin(Edge::Top, super::MARGIN);
        layer.set_layer_shell_margin(Edge::Bottom, super::MARGIN);
        layer.set_size_request(super::WIDTH as i32, super::HEIGHT as i32);
        layer.add(&vbox);
        vbox.show_all();
        layer.connect_map(|w| {
            // Empty input region: every click goes to whatever is underneath.
            // Re-applied on every show, since hiding unmaps the layer surface.
            if let Some(gdk_window) = w.window() {
                gdk_window.input_shape_combine_region(&gtk::cairo::Region::create(), 0, 0);
            }
        });
        LAYER.set(Some(layer));
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use std::cell::Cell;
    use tauri::WebviewWindow;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SetWindowPos, ShowWindow,
    };

    thread_local! {
        // The overlay's HWND (as an integer; HWND isn't Send). Main thread only.
        static OVERLAY: Cell<Option<isize>> = const { Cell::new(None) };
    }

    pub fn setup(window: &WebviewWindow) -> tauri::Result<()> {
        // Click-through: tao adds WS_EX_TRANSPARENT | WS_EX_LAYERED. Done once,
        // here; after this we show and hide the window ourselves and leave
        // tao's style flags alone.
        window.set_ignore_cursor_events(true)?;
        OVERLAY.set(Some(window.hwnd()?.0 as isize));
        Ok(())
    }

    /// Tao's show() can activate the window, which would pull focus out of the
    /// game. SW_SHOWNOACTIVATE never does.
    pub fn set_visible(visible: bool) -> bool {
        let Some(raw) = OVERLAY.get() else { return false };
        let hwnd = HWND(raw as *mut _);
        unsafe {
            let _ = ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
            if visible {
                // Re-assert topmost: a game going fullscreen can push us down.
                let flags = SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE;
                let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
            }
        }
        true
    }

    pub fn set_position(_position: crate::config::OverlayPosition) -> bool {
        false // the generic code positions the window; SetWindowPos keeps no-activate
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    use tauri::WebviewWindow;

    pub fn setup(_window: &WebviewWindow) -> tauri::Result<()> {
        // TODO(macos): NSPanel level above fullscreen spaces.
        Ok(())
    }

    pub fn set_visible(_visible: bool) -> bool {
        false
    }

    pub fn set_position(_position: crate::config::OverlayPosition) -> bool {
        false
    }
}
