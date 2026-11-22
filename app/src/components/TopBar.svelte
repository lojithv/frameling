<script lang="ts">
  import { app, undo, redo, loadProject, eng } from "../lib/store.svelte";
  import { openProject, saveProject, isDesktop } from "../lib/platform";
  import { play, stop } from "../lib/playback";

  let { onExport }: { onExport: () => void } = $props();
  let menu = $state(false);

  function newProject(demo: boolean) {
    menu = false;
    if (app.dirty && !confirm("Discard unsaved changes?")) return;
    const e = eng();
    if (demo) e.load_demo();
    else e.load_blank();
    loadProject(e.project_json(), null);
  }
</script>

<header class="top">
  <div class="brand">
    <img src="/icon.svg" alt="" width="26" height="26" />
    <strong>Frameling</strong>
    <span class="proj">{app.project?.name}{app.dirty ? " •" : ""}</span>
  </div>

  <div class="file">
    <div class="menu-wrap">
      <button onclick={() => (menu = !menu)}>File ▾</button>
      {#if menu}
        <div class="menu" role="menu">
          <button onclick={() => newProject(false)}>New blank project</button>
          <button onclick={() => newProject(true)}>New from demo</button>
          <hr />
          <button onclick={() => { menu = false; openProject(); }}>Open… <kbd>Ctrl O</kbd></button>
          <button onclick={() => { menu = false; saveProject(); }}>Save <kbd>Ctrl S</kbd></button>
          {#if isDesktop}<button onclick={() => { menu = false; saveProject(true); }}>Save as…</button>{/if}
        </div>
        <div class="scrim" onclick={() => (menu = false)} role="presentation"></div>
      {/if}
    </div>
    <button onclick={undo} disabled={!app.canUndo} title="Undo (Ctrl Z)">↶</button>
    <button onclick={redo} disabled={!app.canRedo} title="Redo (Ctrl Shift Z)">↷</button>
  </div>

  <div class="play">
    <button class="pill" onclick={() => (app.playing && app.playMode === "all" ? stop() : play("all"))}>
      {app.playing && app.playMode === "all" ? "❚❚ Stop" : "▶ Play movie"}
    </button>
    <button class="primary" onclick={onExport}>Export video</button>
  </div>
</header>

<style>
  .top {
    display: flex;
    align-items: center;
    gap: 16px;
    height: 52px;
    padding: 0 14px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-1);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 254px;
  }
  .brand strong {
    font-size: 15px;
    letter-spacing: -0.01em;
  }
  .proj {
    color: var(--muted);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 160px;
  }
  .file {
    display: flex;
    gap: 6px;
    flex: 1;
  }
  .file > button {
    width: 34px;
    padding: 6px 0;
  }
  .menu-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    top: 38px;
    left: 0;
    min-width: 220px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 6px;
    z-index: 20;
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.4);
  }
  .menu button {
    display: flex;
    justify-content: space-between;
    width: 100%;
    background: none;
    border: none;
    text-align: left;
    padding: 8px 10px;
  }
  .menu button:hover {
    background: var(--bg-3);
  }
  .menu hr {
    border: none;
    border-top: 1px solid var(--line);
    margin: 4px 0;
  }
  kbd {
    color: var(--muted);
    font-size: 11px;
    font-family: inherit;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 10;
  }
  .play {
    display: flex;
    gap: 8px;
  }
  .pill {
    border-radius: 999px;
    padding: 7px 16px;
  }
</style>
