# Testing checklist

Things to check by hand on each platform, especially before a release. Tick
them off in an issue or PR comment, and note your OS, GPU and game.

## First run

- [ ] The settings window opens on first launch and says the model is missing.
- [ ] Put `ggml-large-v3-turbo-q5_0.bin` where it says. The warning goes away after a restart.
- [ ] The tray icon shows up, and its menu works.

## Dictation

- [ ] Turning on shows the overlay with "Starting up…", then "Ready". The very first
      time this can take ~30 s while the GPU driver compiles shaders.
- [ ] The record hotkey (default Ctrl+Alt+D) plays a chime, and the overlay turns
      red with "Listening…".
- [ ] Pressing it again plays the other chime, then "Transcribing…". Text appears
      in about a second.
- [ ] Hold-to-talk mode (settings): recording runs while you hold the key and stops
      when you let go.
- [ ] "new paragraph" and "scratch that" work.
- [ ] Copy (Ctrl+Alt+C): the text pastes into another app, the draft empties, and
      `archive.txt` has it.
- [ ] Turning off hides the overlay. Task Manager / nvidia-smi shows the VRAM freed.
- [ ] Auto-off: set it to 5 min, leave it alone, and it turns off.

## In a game (the whole point)

Use borderless / windowed fullscreen. Exclusive fullscreen hides the overlay.

- [ ] The overlay is visible over the game.
- [ ] The game keeps focus: showing the overlay, recording and transcribing never
      tab you out or drop keyboard input.
- [ ] Clicks and mouse-look go through the overlay.
- [ ] The hotkeys work while the game is focused. If the game runs as administrator,
      Dictator may need to as well. Note it if so.
- [ ] No noticeable FPS drop while on and idle.

## Settings

- [ ] Changing a hotkey takes effect immediately. A hotkey already taken by
      another app shows an error.
- [ ] Overlay top/bottom and show/hide work.
- [ ] Start on login: log out and back in. Dictator is in the tray and off.
- [ ] Closing the settings window keeps Dictator running in the tray. Quit from
      the tray exits.
