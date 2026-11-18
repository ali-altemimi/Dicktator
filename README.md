# Dictator

Dictate while you game. Press a hotkey, talk, and your words land in an on-screen
draft drawn over the game, without ever taking focus away from it. Copy the draft
into your doc when you're done.

- Fully local: speech-to-text runs on your GPU (whisper.cpp), nothing leaves your machine
- Click-through overlay that sits above fullscreen (borderless) games
- Windows and Linux (including Wayland/Hyprland); macOS later

**Status:** early development. Works end to end on Linux/Wayland. The Windows build
hasn't been tested yet.

## Usage

Dictator lives in the tray and starts **off**: the model isn't loaded and the mic
can't be opened. Turn it on, and the overlay appears with the model ready about 2 s later.

| Default hotkey | Does |
|---|---|
| Ctrl+Alt+D | Start / stop recording (or hold to talk; you can switch in settings) |
| Ctrl+Alt+Shift+D | Turn on / off |
| Ctrl+Alt+C | Copy the draft to the clipboard and clear it (also saved to `archive.txt`) |
| Ctrl+Alt+Z | Undo the last chunk (or say "scratch that") |
| Ctrl+Alt+O | Show / hide the overlay |

You can change all of these in the settings window (tray → Settings…). It also
has auto-off after idle time (default 30 min), start on login, chimes, the
overlay's position and the language.

**Wayland** doesn't let apps grab global keys, so on Wayland you bind the CLI
in your compositor. The settings window generates the Hyprland lines for you.
`dictator <command>` sends the command to the running instance:
`on`, `off`, `power`, `toggle`, `start`, `stop`, `cancel`, `undo`, `copy`,
`overlay`, `settings`, `quit`.

Say "new paragraph" or "new line" while dictating to break up the text.

Files live in `Documents/Dictator/`. `draft.txt` is the draft, and if you add a
`vocab.txt` there, names and jargon in it get spelled correctly more often.

The first time you open it, the settings window offers to download the speech
model (Whisper Large v3 Turbo, 574 MB; smaller ones are there for weaker PCs).
Downloads are checked against pinned SHA-256 hashes. The first "on" on a new
machine can take ~20 s while the GPU driver compiles shaders. After that it takes about 2 s.

The tray icon shows the state: grey when off, a red dot while listening, a yellow dot while busy.

## Development

Needs Rust, Node + pnpm, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
On Linux you also need `gtk-layer-shell`, and Vulkan headers plus `glslc` (shaderc)
for whisper.cpp.

```sh
pnpm install
pnpm tauri dev
pnpm tauri build                            # installers in src-tauri/target/release/bundle
cd src-tauri && cargo test                  # unit tests
# real-model GPU test (see stt.rs for how to make the clip):
DICTATOR_TEST_CLIP=/tmp/s.f32 cargo test -- --ignored --nocapture
```

CI (`.github/workflows/build.yml`) builds and tests on Windows and Linux and uploads
the installers as artifacts. [TESTING.md](TESTING.md) is the manual checklist,
mostly for in-game behaviour that tests can't cover.

## Platform notes

- **Linux / Wayland:** the overlay is a wlr-layer-shell surface (Hyprland, Sway, KDE).
  GNOME has no layer-shell, so the overlay falls back to a normal always-on-top window.
- **Windows:** the overlay is a topmost, click-through window that can't be activated,
  so it never takes focus from the game. Use borderless/windowed fullscreen;
  exclusive fullscreen draws over everything.
- **NVIDIA on Wayland:** the app sets `__NV_DISABLE_EXPLICIT_SYNC=1` at startup. Without it,
  WebKitGTK either crashes with "Missing acquire timeline" or only repaints part of the overlay.

## License

MIT
