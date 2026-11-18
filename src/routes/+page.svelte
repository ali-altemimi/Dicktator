<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import Shortcut from "./Shortcut.svelte";

  // Mirrors config::Settings, engine::Snapshot and lib.rs Environment.
  type Hotkeys = { record: string; power: string; copy: string; undo: string; overlay: string };
  type Settings = {
    hotkeyMode: "toggle" | "hold";
    hotkeys: Hotkeys;
    chimes: boolean;
    overlayEnabled: boolean;
    overlayPosition: "top" | "bottom";
    language: string;
    autoOffMinutes: number;
    startOnLogin: boolean;
    draftDir: string | null;
    model: string | null;
  };
  type Status = "off" | "loading" | "ready" | "recording" | "transcribing";
  type Snapshot = { status: Status; text: string; words: number; msg: string };
  type HotkeyStatus = { native: boolean; errors: string[] };
  type Env = {
    wayland: boolean;
    exe: string;
    settingsFile: string;
    draftDir: string;
    model: string;
    modelFound: boolean;
  };

  let settings = $state<Settings | null>(null);
  let env = $state<Env | null>(null);
  let hotkeyStatus = $state<HotkeyStatus>({ native: true, errors: [] });
  let status = $state<Status>("off");
  let words = $state(0);
  let msg = $state("");
  let saveError = $state("");
  type ModelEntry = { id: string; label: string; note: string; bytes: number; downloaded: boolean; inUse: boolean };
  let models = $state<ModelEntry[]>([]);
  let download = $state<{ id: string; done: number; total: number } | null>(null);
  let downloadError = $state("");
  let copied = $state(false);

  const statusLabel: Record<Status, string> = {
    off: "Off",
    loading: "Starting up…",
    ready: "Ready",
    recording: "Listening…",
    transcribing: "Transcribing…",
  };

  const hotkeyNames: [keyof Hotkeys, string][] = [
    ["record", "Start / stop recording"],
    ["power", "Turn on / off"],
    ["copy", "Copy draft and clear"],
    ["undo", "Undo last chunk"],
    ["overlay", "Show / hide overlay"],
  ];

  const languages: [string, string][] = [
    ["en", "English"],
    ["auto", "Detect automatically"],
    ["de", "German"],
    ["es", "Spanish"],
    ["fr", "French"],
    ["it", "Italian"],
    ["nl", "Dutch"],
    ["pt", "Portuguese"],
    ["pl", "Polish"],
    ["ar", "Arabic"],
    ["ja", "Japanese"],
    ["zh", "Chinese"],
  ];

  onMount(() => {
    invoke<Settings>("get_settings").then((s) => (settings = s));
    invoke<Env>("environment").then((e) => (env = e));
    invoke<HotkeyStatus>("get_hotkey_status").then((h) => (hotkeyStatus = h));
    invoke<Snapshot | null>("get_state").then((s) => s && applyState(s));
    invoke<ModelEntry[]>("list_models").then((m) => (models = m));
    const unlisten = [
      listen<Snapshot>("state", (e) => applyState(e.payload)),
      listen<{ id: string; done: number; total: number }>("model-download", (e) => (download = e.payload)),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });

  function applyState(s: Snapshot) {
    status = s.status;
    words = s.words;
    if (s.msg) msg = s.msg;
  }

  // Every change saves immediately; no Save button to forget.
  async function save() {
    if (!settings) return;
    try {
      hotkeyStatus = await invoke<HotkeyStatus>("save_settings", { settings: $state.snapshot(settings) });
      env = await invoke<Env>("environment");
      saveError = "";
    } catch (e) {
      saveError = String(e);
    }
  }

  const send = (command: string) => invoke("send_command", { command });

  const mb = (bytes: number) => `${Math.round(bytes / 1e6)} MB`;

  async function refreshModels() {
    models = await invoke<ModelEntry[]>("list_models");
    env = await invoke<Env>("environment");
  }

  async function downloadModel(id: string, use: boolean) {
    downloadError = "";
    download = { id, done: 0, total: 1 };
    try {
      await invoke("download_model", { id });
      if (use) await invoke("use_model", { id });
    } catch (e) {
      if (!String(e).includes("cancelled")) downloadError = String(e);
    }
    download = null;
    await refreshModels();
  }

  async function useModel(id: string) {
    try {
      await invoke("use_model", { id });
      settings = await invoke<Settings>("get_settings");
    } catch (e) {
      downloadError = String(e);
    }
    await refreshModels();
  }

  // Compositor keybind lines for Wayland, in Hyprland's syntax.
  function hyprKey(shortcut: string): string {
    const parts = shortcut.split("+");
    const key = parts.pop() ?? "";
    const mods = parts.map((m) => (m === "Ctrl" ? "CTRL" : m === "Super" ? "SUPER" : m.toUpperCase()));
    return `${mods.join(" ")}, ${key}`;
  }

  let bindLines = $derived.by(() => {
    if (!settings || !env) return "";
    const exe = env.exe;
    const k = settings.hotkeys;
    const lines = [];
    if (k.record) {
      if (settings.hotkeyMode === "hold") {
        lines.push(`bind = ${hyprKey(k.record)}, exec, ${exe} start`);
        lines.push(`bindr = ${hyprKey(k.record)}, exec, ${exe} stop`);
      } else {
        lines.push(`bind = ${hyprKey(k.record)}, exec, ${exe} toggle`);
      }
    }
    const rest: [keyof Hotkeys, string][] = [
      ["power", "power"],
      ["copy", "copy"],
      ["undo", "undo"],
      ["overlay", "overlay"],
    ];
    for (const [key, cmd] of rest) if (k[key]) lines.push(`bind = ${hyprKey(k[key])}, exec, ${exe} ${cmd}`);
    return lines.join("\n");
  });

  async function copyBinds() {
    await navigator.clipboard.writeText(bindLines);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
</script>

<main>
  <header>
    <h1>Dictator</h1>
    <span class="pill {status}">{statusLabel[status]}</span>
    <span class="muted">{words} words in draft</span>
    <div class="spacer"></div>
    <button onclick={() => send("power")}>{status === "off" ? "Turn on" : "Turn off"}</button>
    <button
      class="primary"
      disabled={status === "off" || status === "loading"}
      onclick={() => send("toggle")}>{status === "recording" ? "Stop" : "Record"}</button>
  </header>
  {#if msg}<p class="msg">{msg}</p>{/if}

  {#if env && !env.modelFound}
    <div class="warn">
      <strong>Dictator needs a speech model before it can turn on.</strong>
      It runs on your computer, so nothing you say leaves it. It's a one-time download.
      {#if !download}
        <div><button class="primary" onclick={() => downloadModel("turbo", true)}>Download the recommended model ({mb(models[0]?.bytes ?? 574e6)})</button></div>
      {/if}
    </div>
  {/if}

  {#if settings}
    <section>
      <h2>Hotkeys</h2>
      <div class="row">
        <span>Recording key</span>
        <label><input type="radio" bind:group={settings.hotkeyMode} value="toggle" onchange={save} /> Press to start, press again to stop</label>
        <label><input type="radio" bind:group={settings.hotkeyMode} value="hold" onchange={save} /> Hold to talk</label>
      </div>

      {#each hotkeyNames as [key, label]}
        <div class="row">
          <span>{label}</span>
          <Shortcut bind:value={settings.hotkeys[key]} onchange={save} />
        </div>
      {/each}
      {#each hotkeyStatus.errors as err}<p class="error">Couldn't register {err}</p>{/each}

      {#if env?.wayland}
        <div class="note">
          <p>
            <strong>On Wayland, apps can't grab global keys.</strong> Bind the keys in your compositor instead. For
            Hyprland, add these lines to your config (other desktops: make a custom shortcut that runs the same
            command):
          </p>
          <pre>{bindLines}</pre>
          <button onclick={copyBinds}>{copied ? "Copied" : "Copy lines"}</button>
        </div>
      {/if}
    </section>

    <section>
      <h2>Overlay</h2>
      <label class="check"><input type="checkbox" bind:checked={settings.overlayEnabled} onchange={save} /> Show the overlay while Dictator is on</label>
      <div class="row">
        <span>Position</span>
        <select bind:value={settings.overlayPosition} onchange={save}>
          <option value="top">Top of the screen</option>
          <option value="bottom">Bottom of the screen</option>
        </select>
      </div>
      <p class="hint">Games need to run in borderless or windowed mode for the overlay to show on top.</p>
    </section>

    <section>
      <h2>General</h2>
      <label class="check"><input type="checkbox" bind:checked={settings.chimes} onchange={save} /> Play a sound when recording starts and stops</label>
      <label class="check"><input type="checkbox" bind:checked={settings.startOnLogin} onchange={save} /> Start Dictator when I log in (it starts off)</label>
      <div class="row">
        <span>Turn off when unused for</span>
        <select bind:value={settings.autoOffMinutes} onchange={save}>
          <option value={0}>Never</option>
          <option value={5}>5 minutes</option>
          <option value={15}>15 minutes</option>
          <option value={30}>30 minutes</option>
          <option value={60}>1 hour</option>
        </select>
      </div>
      <p class="hint">Turning off frees the speech model's memory (about 1 GB of VRAM) and closes the mic.</p>
      <div class="row">
        <span>Language</span>
        <select bind:value={settings.language} onchange={save}>
          {#each languages as [code, name]}<option value={code}>{name}</option>{/each}
        </select>
      </div>
    </section>

    <section>
      <h2>Speech model</h2>
      {#each models as m}
        <div class="model" class:in-use={m.inUse}>
          <div class="model-text">
            <strong>{m.label}</strong> <span class="muted">{mb(m.bytes)}</span>
            {#if m.inUse}<span class="pill ready">In use</span>{/if}
            <div class="hint">{m.note}</div>
            {#if download?.id === m.id}
              <progress value={download.done} max={download.total}></progress>
              <span class="muted">{mb(download.done)} of {mb(download.total)}</span>
            {/if}
          </div>
          {#if download?.id === m.id}
            <button onclick={() => invoke("cancel_download")}>Cancel</button>
          {:else if m.downloaded && !m.inUse}
            <button onclick={() => useModel(m.id)}>Use</button>
          {:else if !m.downloaded}
            <button disabled={!!download} onclick={() => downloadModel(m.id, !models.some((x) => x.inUse && x.downloaded))}>Download</button>
          {/if}
        </div>
      {/each}
      {#if downloadError}<p class="error">{downloadError}</p>{/if}
      <p class="hint">A new model is used from the next time you turn Dictator on. Models are saved in <code>{env?.model?.replace(/[^/\\]*$/, "")}</code></p>
    </section>

    <section>
      <h2>Draft</h2>
      <div class="row">
        <span>Folder</span>
        <code class="path">{env?.draftDir}</code>
        <button onclick={() => invoke("open_folder")}>Open</button>
      </div>
      <p class="hint">
        <code>draft.txt</code> is what you've dictated. Copying moves it to the clipboard and keeps a copy in
        <code>archive.txt</code>. Put names and jargon in <code>vocab.txt</code> (comma-separated) and they'll be
        spelled right more often. Say "new paragraph", "new line" or "scratch that" while dictating.
      </p>
    </section>

    {#if saveError}<p class="error">{saveError}</p>{/if}
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: Inter, system-ui, sans-serif;
    font-size: 14px;
    color: #1d1d22;
    background: #f6f6f8;
  }
  @media (prefers-color-scheme: dark) {
    :global(body) { color: #e8e8ee; background: #18181c; }
    section { background: #222228 !important; }
    .note, .warn { background: #2a2a31 !important; }
    pre, code { background: #2f2f37 !important; }
    button, select { background: #2f2f37 !important; color: inherit !important; border-color: #3a3a44 !important; }
  }
  main { max-width: 640px; margin: 0 auto; padding: 18px 20px 30px; }
  header { display: flex; align-items: center; gap: 10px; }
  h1 { font-size: 20px; margin: 0; }
  h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.06em; color: #8a8a96; margin: 0 0 10px; }
  .spacer { flex: 1; }
  .muted, .hint { color: #8a8a96; }
  .hint { font-size: 12.5px; margin: 6px 0 0; line-height: 1.5; }
  .msg { color: #2b8ac6; margin: 8px 0 0; }
  .error { color: #d2383d; margin: 6px 0 0; }
  .pill { padding: 2px 10px; border-radius: 99px; font-size: 12px; font-weight: 600; background: #ddd; color: #555; }
  .pill.ready { background: #d8f0dd; color: #23733a; }
  .pill.recording { background: #fbd9da; color: #b3262b; }
  .pill.loading, .pill.transcribing { background: #fbeec6; color: #8a6510; }
  section { background: #fff; border-radius: 10px; padding: 14px 16px; margin-top: 14px; }
  .row { display: flex; align-items: center; gap: 12px; margin: 8px 0; flex-wrap: wrap; }
  .row > span:first-child { width: 190px; flex: none; }
  label.check { display: block; margin: 8px 0; }
  button, select {
    font: inherit; padding: 5px 12px; border-radius: 7px; border: 1px solid #d4d4dc; background: #fff; cursor: pointer;
  }
  button.primary { background: #e5484d !important; color: #fff !important; border-color: #e5484d !important; }
  button:disabled { opacity: 0.45; cursor: default; }
  .note, .warn { background: #f1f1f5; border-radius: 8px; padding: 10px 12px; margin-top: 10px; line-height: 1.5; }
  .warn { border-left: 3px solid #f5c542; margin-top: 14px; }
  .note p { margin: 0 0 8px; }
  pre { font-size: 12px; background: #e9e9ef; padding: 8px 10px; border-radius: 6px; overflow-x: auto; margin: 0 0 8px; }
  code { font-size: 12px; background: #ececf1; padding: 1px 5px; border-radius: 4px; word-break: break-all; }
  code.path { flex: 1; }
  .model { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-top: 1px solid rgba(128, 128, 140, 0.18); }
  .model:first-of-type { border-top: none; }
  .model-text { flex: 1; }
  .model .pill { margin-left: 6px; }
  progress { width: 60%; vertical-align: middle; margin-right: 8px; }
  .warn button { margin-top: 8px; }
</style>
