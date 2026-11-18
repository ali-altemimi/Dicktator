<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  // Mirrors engine::Snapshot in src-tauri/src/engine.rs.
  type Status = "off" | "loading" | "ready" | "recording" | "transcribing";
  type Snapshot = { status: Status; text: string; words: number; msg: string };

  let status = $state<Status>("off");
  let text = $state("");
  let words = $state(0);
  let msg = $state("");
  let msgTimer: ReturnType<typeof setTimeout> | undefined;
  let position = $state<"top" | "bottom">("top");

  const label: Record<Status, string> = {
    off: "Off",
    loading: "Starting up…",
    ready: "Ready",
    recording: "Listening…",
    transcribing: "Transcribing…",
  };

  function apply(s: Snapshot) {
    status = s.status;
    text = s.text;
    words = s.words;
    if (s.msg) {
      msg = s.msg;
      clearTimeout(msgTimer);
      msgTimer = setTimeout(() => (msg = ""), 4000);
    }
  }

  onMount(() => {
    type Settings = { overlayPosition: "top" | "bottom" };
    const unlisten = [
      listen<Snapshot>("state", (e) => apply(e.payload)),
      listen<Settings>("settings", (e) => (position = e.payload.overlayPosition)),
    ];
    invoke<Snapshot | null>("get_state").then((s) => s && apply(s));
    invoke<Settings>("get_settings").then((s) => (position = s.overlayPosition));
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });
</script>

<!-- The window is a fixed size; hug its top or bottom edge to match the position. -->
<div class="frame {position}">
<div class="box {status}" class:dim={status === "ready" && !msg}>
  <header>
    <span class="dot {status}"></span>
    <strong>{label[status]}</strong>
    <span class="muted">· {words} words</span>
    {#if msg}<span class="msg">· {msg}</span>{/if}
  </header>
  <!-- Bottom-aligned and clipped at the top, so the newest line is always visible. -->
  <div class="body">
    <p class:empty={!text.trim()}>{text.trim() ? text : "(empty, dictated text shows up here)"}</p>
  </div>
</div>
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    font-family: Inter, system-ui, sans-serif;
  }
  .frame { height: 100vh; display: flex; flex-direction: column; }
  .frame.bottom { justify-content: flex-end; }
  .box {
    box-sizing: border-box;
    padding: 10px 14px 12px;
    border-radius: 10px;
    background: rgba(15, 15, 20, 0.72);
    border: 2px solid transparent;
    color: #f2f2f5;
    transition: opacity 200ms, border-color 200ms;
  }
  .box.dim { opacity: 0.6; }
  .box.recording { border-color: #e5484d; }
  header { display: flex; align-items: center; gap: 8px; font-size: 13px; white-space: nowrap; }
  .muted { color: #9a9aa6; }
  .msg { color: #7fd1ff; overflow: hidden; text-overflow: ellipsis; }
  .dot { flex: none; width: 10px; height: 10px; border-radius: 50%; background: #6b6b76; }
  .dot.recording { background: #e5484d; animation: pulse 1s infinite; }
  .dot.transcribing, .dot.loading { background: #f5c542; animation: pulse 1s infinite; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  .body {
    margin-top: 6px;
    max-height: calc(6 * 1.4em);
    font-size: 15px;
    line-height: 1.4;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
  }
  p { margin: 0; white-space: pre-wrap; }
  p.empty { opacity: 0.5; }
</style>
