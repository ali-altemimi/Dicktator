# Windows test report: 2026-11-27

First hands-on run of Dictator on Windows. It follows [TESTING.md](../TESTING.md) as far as the machine allowed.

## Setup

| | |
|---|---|
| OS | Windows 10 Pro 22H2 (10.0.19045) |
| GPU | NVIDIA GeForce RTX 3060 (Vulkan) |
| Build tested | CI artifact `dictator-Windows` from run [45534040187](https://github.com/ali-altemimi/Dicktator/actions/runs/45534040187) (`main`, "README: install instructions") |
| Installer | `Dictator_0.1.0_x64-setup.exe` (NSIS), installed silently with `/S` |
| Model | `ggml-large-v3-turbo-q5_0.bin`, downloaded by hand into `%LOCALAPPDATA%\dictator\models`; its SHA-256 matched the pinned hash |
| Microphone | **None.** The only capture devices are virtual (Steam Streaming Microphone, DroidCam) |

The app was driven by its CLI (`dictator.exe on|start|stop|copy|overlay|off`) and by synthesized
keypresses. Screenshots were taken to check the UI, and the Win32 APIs were used to check
focus and window styles.

## Results

| Area | Check | Result |
|---|---|---|
| Install | Per-user install, no admin/UAC prompt | ✅ Installed to `%LOCALAPPDATA%\Dictator` |
| First run | Settings window opens and says the model is missing | ✅ |
| First run | Banner clears once the model is in place | ⚠️ Needed a restart (bug 2) |
| Power | `on` shows the overlay, "Starting up…" then "Ready" | ✅ Ready in under 30 s, no long shader-compile pause seen |
| Power | Model is on the GPU | ✅ VRAM 790 → 1329 MiB (about +540 MB) |
| Power | `off` frees VRAM | ✅ 1336 → 805 MiB; UI shows "Off. Mic closed, model unloaded" |
| Recording | `start` turns the overlay red with "Listening…" | ✅ |
| Recording | `stop` goes to "Transcribing…" then back to Ready | ✅ Result was "Heard nothing", because there is no real mic (see below) |
| Recording | Speech gets transcribed into the draft | ❓ **Not tested**: no physical microphone |
| Hotkeys | Ctrl+Alt+D (global) starts and stops recording from another app | ✅ |
| Copy | `copy` puts the draft on the clipboard, appends it to `archive.txt` and empties `draft.txt` | ✅ |
| Overlay | `overlay` hides it and saves `overlayEnabled: false` | ✅ |
| Overlay | Settings checkbox reflects the change | ❌ Stayed ticked (bug 1) |
| Overlay | Never takes focus | ✅ Notepad stayed the foreground window through on, start and stop |
| Overlay | Click-through and topmost | ✅ ex-style `0x080C0138`: `WS_EX_TOPMOST`, `WS_EX_TRANSPARENT`, `WS_EX_NOACTIVATE`, `WS_EX_LAYERED` |
| Single instance | Launching the exe again doesn't start a second process | ✅ |

### Not covered

- **Real transcription.** The machine has no physical microphone. Text-to-speech played through the
  speakers didn't reach either virtual capture device. Still to do: plug in a headset, press
  Ctrl+Alt+D, speak, then try "new paragraph" and "scratch that".
- **Hold-to-talk mode, auto-off, start on login, chimes by ear, changing a hotkey or a clash with one
  another app holds.**
- **The in-game checklist** (borderless game, mouse-look through the overlay, FPS, a game running as admin).
- **Building from source and `cargo test` on Windows.** None of the toolchain (Rust, CMake, Vulkan SDK,
  LLVM) is installed here. CI already builds and runs the unit tests on `windows-latest`.

## Bugs found

### 1. Settings window ignores changes made elsewhere (fixed in this commit)

Toggling the overlay from the tray, Ctrl+Alt+O or `dictator overlay` saves the setting, and
`apply_settings` emits a `settings` event. `src/routes/+page.svelte` never listened for that
event, so the "Show the overlay" checkbox stayed out of date. Worse, the next change made in the
window saves the whole stale `settings` object and turns the overlay back on.

**Fix:** listen for `settings` and replace the local copy.

### 2. "Needs a speech model" banner stays up until restart (fixed in this commit)

The page only re-checks `environment` on mount, after a save and after an in-app download. If the
model is copied in by hand (as TESTING.md describes), the banner stays up until the app restarts.

**Fix:** re-check models and environment when the window gains focus or becomes visible, for example
when it's reopened from the tray.

### Verification of the fixes

- `pnpm check` (svelte-check): 0 errors, 0 warnings. `pnpm build` succeeds.
- **Not run in the app yet.** The UI is embedded in the binary, and rebuilding it needs the full
  Rust and Vulkan toolchain. To check it in `pnpm tauri dev`:
  1. Open Settings.
  2. Run `dictator overlay`. The checkbox should untick.
  3. With no model present, open Settings, copy a model into the models folder, then click back into
     the window. The banner should go away.

## Minor observations

- The overlay window doesn't have `WS_EX_TOOLWINDOW` (it has `WS_EX_APPWINDOW`). `skip_taskbar(true)`
  is set, so it shouldn't show in the taskbar, but check whether it shows up in Alt+Tab.
- The release build uses `windows_subsystem = "windows"`, so `dictator.exe <badcommand>` prints
  nothing on Windows: the "unknown command" error goes nowhere.
