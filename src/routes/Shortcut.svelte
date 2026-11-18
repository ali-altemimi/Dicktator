<!--
  A field that records a key combination. Click it, press the keys.
  Esc cancels, Backspace/Delete unbinds. Produces tauri-plugin-global-shortcut
  syntax, e.g. "Ctrl+Alt+D".
-->
<script lang="ts">
  let { value = $bindable(""), onchange }: { value: string; onchange?: () => void } = $props();
  let listening = $state(false);

  const MODIFIERS = ["Control", "Alt", "Shift", "Meta", "OS", "AltGraph"];

  // KeyboardEvent.code -> plugin key name. Codes are layout-independent, so
  // the shortcut stays on the same physical key on any keyboard layout.
  function keyName(code: string): string {
    if (code.startsWith("Key")) return code.slice(3); // KeyD -> D
    if (code.startsWith("Digit")) return code.slice(5); // Digit1 -> 1
    return code; // F8, Space, Numpad1, Backquote ... the plugin knows these
  }

  function onkeydown(e: KeyboardEvent) {
    if (!listening) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") return stop();
    if ((e.key === "Backspace" || e.key === "Delete") && !e.ctrlKey && !e.altKey && !e.metaKey) {
      return commit("");
    }
    if (MODIFIERS.includes(e.key)) return; // wait for the actual key
    const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"];
    commit([...mods.filter(Boolean), keyName(e.code)].join("+"));
  }

  function commit(v: string) {
    value = v;
    stop();
    onchange?.();
  }

  function stop() {
    listening = false;
  }
</script>

<button
  class="shortcut"
  class:listening
  onclick={() => (listening = true)}
  onblur={stop}
  {onkeydown}
  title="Click, then press the keys. Backspace to unbind."
>
  {#if listening}Press keys…{:else if value}{value}{:else}<span class="unset">Not set</span>{/if}
</button>

<style>
  .shortcut { min-width: 170px; text-align: left; font-family: ui-monospace, monospace; font-size: 13px; }
  .listening { border-color: #e5484d !important; color: #e5484d !important; }
  .unset { opacity: 0.5; font-family: Inter, system-ui, sans-serif; }
</style>
