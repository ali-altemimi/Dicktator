# Dictator

Dictate while you game. Press a hotkey, talk, and your words land in an on-screen
draft drawn over the game, without ever taking focus away from it. Copy the draft
into your doc when you're done.

- Fully local: speech-to-text runs on your GPU (whisper.cpp), nothing leaves your machine
- Click-through overlay that sits above fullscreen (borderless) games
- Windows and Linux (including Wayland/Hyprland); macOS later

**Status:** early development. Works end to end on Linux/Wayland, driven from the
command line. There's no built-in hotkey, tray icon or settings window yet.

## Usage

Dictator starts **off**: the model isn't loaded and the mic can't be opened. Control
it by running `dictator <command>`. If it's already running, the command goes to
the running instance:

| Command | Does |
|---|---|
| `on` / `off` / `power` | Load the model and show the overlay / unload and hide / switch between the two |
| `toggle` | Start or stop recording (`start`, `stop`, `cancel` also exist) |
| `undo` | Remove the last dictated chunk (or say "scratch that") |
| `copy` | Copy the draft to the clipboard and clear it (it's also saved to `archive.txt`) |

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
