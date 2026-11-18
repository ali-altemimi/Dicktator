# Dictator

Dictate while you game. Press a hotkey, talk, and your words land in an on-screen
draft drawn over the game, without ever taking focus away from it. Copy the draft
into your doc when you're done.

- Fully local: speech-to-text runs on your GPU (whisper.cpp), nothing leaves your machine
- Click-through overlay that sits above fullscreen (borderless) games
- Windows and Linux (including Wayland/Hyprland); macOS later

**Status:** early development. The overlay works on Linux/Wayland. Speech-to-text
is next.

## Development

Needs Rust, Node + pnpm, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
On Linux you also need `gtk-layer-shell`.

```sh
pnpm install
pnpm tauri dev
```

## Platform notes

- **Linux / Wayland:** the overlay is a wlr-layer-shell surface (Hyprland, Sway, KDE).
  GNOME has no layer-shell, so the overlay falls back to a normal always-on-top window.
- **NVIDIA on Wayland:** the app sets `__NV_DISABLE_EXPLICIT_SYNC=1` at startup. Without it,
  WebKitGTK either crashes with "Missing acquire timeline" or only repaints part of the overlay.

## License

MIT
