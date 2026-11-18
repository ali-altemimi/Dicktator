<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type State = { status: "idle" | "recording" | "transcribing"; text: string };

  let status = $state<State["status"]>("idle");
  let text = $state("");
  let words = $derived(text.split(/\s+/).filter(Boolean).length);

  const label = {
    idle: "Ready",
    recording: "Listening…",
    transcribing: "Transcribing…",
  };

  onMount(() => {
    const unlisten = listen<State>("state", (e) => {
      status = e.payload.status;
      text = e.payload.text;
    });
    return () => void unlisten.then((f) => f());
  });
</script>

<div class="box" class:recording={status === "recording"} class:idle={status === "idle"}>
  <header>
    <span class="dot {status}"></span>
    <strong>{label[status]}</strong>
    <span class="muted">· {words} words</span>
  </header>
  <!-- Bottom-aligned and clipped at the top, so the newest line is always visible. -->
  <div class="body">
    <p class:empty={!text.trim()}>{text.trim() ? text.slice(-1500) : "(empty, dictated text shows up here)"}</p>
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
  .box {
    box-sizing: border-box;
    margin: 0;
    padding: 10px 14px 12px;
    border-radius: 10px;
    background: rgba(15, 15, 20, 0.72);
    border: 2px solid transparent;
    color: #f2f2f5;
    transition: opacity 200ms, border-color 200ms;
  }
  .box.idle { opacity: 0.6; }
  .box.recording { border-color: #e5484d; }
  header { display: flex; align-items: center; gap: 8px; font-size: 13px; }
  .muted { color: #9a9aa6; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: #6b6b76; }
  .dot.recording { background: #e5484d; animation: pulse 1s infinite; }
  .dot.transcribing { background: #f5c542; animation: pulse 1s infinite; }
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
