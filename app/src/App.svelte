<script lang="ts">
  import { onMount } from "svelte";
  import { app, boot, undo, redo, run, currentScene, selectedItem, toast } from "./lib/store.svelte";
  import { nudgeSelected, duplicateSelected, copySelected, cutSelected, paste, hasClip } from "./lib/edit.svelte";
  import { toggle } from "./lib/playback";
  import { openProject, saveProject, resumeFile, watchFileForAgents } from "./lib/platform";
  import TopBar from "./components/TopBar.svelte";
  import Library from "./components/Library.svelte";
  import Stage from "./components/Stage.svelte";
  import Inspector from "./components/Inspector.svelte";
  import Timeline from "./components/Timeline.svelte";
  import ExportDialog from "./components/ExportDialog.svelte";
  import CharacterCreator from "./components/CharacterCreator.svelte";
  import TemplatePicker from "./components/TemplatePicker.svelte";
  import ScriptDialog from "./components/ScriptDialog.svelte";

  let exportOpen = $state(false);
  let templatesOpen = $state(false);
  let scriptOpen = $state(false);
  let error = $state<string | null>(null);

  onMount(() => {
    boot()
      .then(resumeFile)
      .then(watchFileForAgents)
      .catch((e) => (error = String(e)));
  });

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    const typing = t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "s") {
      e.preventDefault();
      saveProject();
      return;
    }
    if (mod && e.key.toLowerCase() === "o") {
      e.preventDefault();
      openProject();
      return;
    }
    if (typing) return;
    const scene = currentScene();
    const item = selectedItem();
    if (mod && e.key.toLowerCase() === "z") {
      e.preventDefault();
      e.shiftKey ? redo() : undo();
    } else if (mod && e.key.toLowerCase() === "y") {
      e.preventDefault();
      redo();
    } else if (e.key === " ") {
      e.preventDefault();
      toggle();
    } else if ((e.key === "Delete" || e.key === "Backspace") && item && scene) {
      run({ op: "removeItem", scene: scene.id, id: item.id });
    } else if (mod && e.key.toLowerCase() === "d" && item && scene) {
      e.preventDefault();
      duplicateSelected();
    } else if (mod && e.key.toLowerCase() === "c" && item) {
      if (window.getSelection()?.toString()) return; // copying page text
      e.preventDefault();
      copySelected();
    } else if (mod && e.key.toLowerCase() === "x" && item && scene) {
      e.preventDefault();
      cutSelected();
    } else if (mod && e.key.toLowerCase() === "v" && hasClip()) {
      e.preventDefault();
      paste();
    } else if (e.key.startsWith("Arrow") && !mod && item && scene && !item.locked) {
      e.preventDefault();
      const step = e.shiftKey ? 10 : 1;
      const dx = e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
      const dy = e.key === "ArrowUp" ? -step : e.key === "ArrowDown" ? step : 0;
      nudgeSelected(dx, dy);
    } else if (e.key === "Escape") {
      app.selected = null;
    } else if (e.key === "," || e.key === ".") {
      const d = e.key === "," ? -1 : 1;
      app.frame = Math.max(0, Math.min((scene?.duration ?? 1) - 1, app.frame + d * (e.shiftKey ? 10 : 1)));
    }
  }

  function onBeforeUnload(e: BeforeUnloadEvent) {
    // Work is autosaved, but warn about unsaved files anyway.
    if (app.dirty) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} onbeforeunload={onBeforeUnload} />

{#if error}
  <div class="boot">Could not start the engine: {error}</div>
{:else if !app.ready}
  <div class="boot"><div class="spinner"></div>Loading Frameling…</div>
{:else}
  <div class="shell">
    <TopBar onExport={() => (exportOpen = true)} onTemplates={() => (templatesOpen = true)} onScript={() => (scriptOpen = true)} />
    <div class="main">
      <Library />
      <div class="center">
        <Stage />
        <Timeline />
      </div>
      <Inspector />
    </div>
  </div>
  <ExportDialog bind:open={exportOpen} />
  <TemplatePicker bind:open={templatesOpen} />
  <ScriptDialog bind:open={scriptOpen} />
  {#if app.creator}
    {#key app.creator}<CharacterCreator />{/key}
  {/if}
{/if}

{#if app.toast}
  <div class="toast {app.toast.kind}" role="status">{app.toast.text}</div>
{/if}

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .center {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .boot {
    height: 100vh;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 14px;
    color: var(--muted);
  }
  .spinner {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid var(--bg-3);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .toast {
    position: fixed;
    bottom: 22px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-3);
    border: 1px solid var(--line);
    padding: 10px 16px;
    border-radius: 10px;
    font-size: 13px;
    z-index: 60;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.4);
    animation: rise 0.18s ease-out;
  }
  .toast.error {
    border-color: #e84a5f;
    color: #ffb3bd;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translate(-50%, 8px);
    }
  }
</style>
