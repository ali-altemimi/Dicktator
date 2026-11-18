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

Model: put [`ggml-large-v3-turbo-q5_0.bin`](https://huggingface.co/ggerganov/whisper.cpp/blob/main/ggml-large-v3-turbo-q5_0.bin)
in `~/.local/share/dictator/models/` (Linux) or `%APPDATA%\dictator\models\` (Windows).
For now you download it by hand. The first `on` on a new machine takes about 20 s
while the GPU driver compiles shaders; after that it takes about 2 s.

## Development

Needs Rust, Node + pnpm, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
On Linux you also need `gtk-layer-shell`, and Vulkan headers plus `glslc` (shaderc)
for whisper.cpp.

```sh
pnpm install
pnpm tauri dev
cd src-tauri && cargo test                  # unit tests
# real-model GPU test (see stt.rs for how to make the clip):
DICTATOR_TEST_CLIP=/tmp/s.f32 cargo test -- --ignored --nocapture
```

## Platform notes

- **Linux / Wayland:** the overlay is a wlr-layer-shell surface (Hyprland, Sway, KDE).
  GNOME has no layer-shell, so the overlay falls back to a normal always-on-top window.
- **NVIDIA on Wayland:** the app sets `__NV_DISABLE_EXPLICIT_SYNC=1` at startup. Without it,
  WebKitGTK either crashes with "Missing acquire timeline" or only repaints part of the overlay.

## License

MIT
