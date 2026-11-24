<script lang="ts">
  import { app, undo, redo, loadProject, eng, forgetRecent } from "../lib/store.svelte";
  import { openProject, openPath, saveProject, isDesktop } from "../lib/platform";
  import { play, stop } from "../lib/playback";

  let { onExport, onTemplates, onScript }: { onExport: () => void; onTemplates: () => void; onScript: () => void } = $props();
  let menu = $state(false);

  function newProject(demo: boolean) {
    menu = false;
    if (app.dirty && !confirm("Discard unsaved changes?")) return;
    const e = eng();
    if (demo) e.load_demo();
    else e.load_blank();
    loadProject(e.project_json(), null);
  }

  let recentOpen = $state(false);
  async function openRecent(path: string) {
    menu = false;
    recentOpen = false;
    if (path === app.filePath && !app.dirty) return;
    if (app.dirty && !confirm("Discard unsaved changes?")) return;
    await openPath(path);
  }
  /** "~/Documents/frameling-projects" style folder label. */
  function folderOf(path: string) {
    const dir = path.replace(/[\\/][^\\/]*$/, "");
    return dir.replace(/^\/Users\/[^/]+/, "~").replace(/^[A-Za-z]:\\Users\\[^\\]+/, "~");
  }
  function ago(ms: number) {
    const m = Math.round((Date.now() - ms) / 60000);
    if (m < 1) return "just now";
    if (m < 60) return `${m} min ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h} h ago`;
    const d = Math.round(h / 24);
    return d === 1 ? "yesterday" : `${d} days ago`;
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
      <button onclick={() => { menu = !menu; recentOpen = false; }}>File ▾</button>
      {#if menu}
        <div class="menu" role="menu">
          <button onclick={() => newProject(false)}>New blank project</button>
          <button onclick={() => { menu = false; onTemplates(); }}>New from template…</button>
          <button onclick={() => { menu = false; onScript(); }}>New from a script…</button>
          <button onclick={() => newProject(true)}>New from demo</button>
          <hr />
          <button onclick={() => { menu = false; openProject(); }}>Open… <kbd>Ctrl O</kbd></button>
          {#if isDesktop}
            <button class="sub" class:on={recentOpen} onclick={() => (recentOpen = !recentOpen)}>Recent projects <kbd>{app.recent.length || ""} ▸</kbd></button>
            {#if recentOpen}
              <div class="recent" role="menu">
                {#each app.recent as r (r.path)}
                  <div class="row" class:current={r.path === app.filePath}>
                    <button class="open" onclick={() => openRecent(r.path)} title={r.path}>
                      <span class="nm">{r.name}</span>
                      <span class="where">{folderOf(r.path)} · {ago(r.at)}</span>
                    </button>
                    <button class="x" title="Remove from this list (the file stays)" aria-label="Remove {r.name} from recent projects" onclick={() => forgetRecent(r.path)}>✕</button>
                  </div>
                {:else}
                  <p class="empty">Projects you open or save will show up here.</p>
                {/each}
                {#if app.recent.length}
                  <hr />
                  <button class="clear" onclick={() => forgetRecent()}>Clear list</button>
                {/if}
              </div>
            {/if}
          {/if}
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
    <button class="pill" onclick={onScript} title="Paste a dialogue script and get a voiced, animated video">✍ Script to video</button>
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
  .menu .sub.on {
    background: var(--bg-3);
  }
  .recent {
    position: absolute;
    left: calc(100% + 6px);
    top: 70px;
    width: 340px;
    max-height: 420px;
    overflow-y: auto;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 6px;
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.4);
  }
  .recent .row {
    display: flex;
    align-items: center;
    border-radius: 7px;
  }
  .recent .row:hover {
    background: var(--bg-3);
  }
  .recent .row.current .nm::after {
    content: " · open";
    color: var(--accent-2);
    font-weight: 400;
  }
  .menu .recent .open {
    flex: 1;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 0;
  }
  .menu .recent .open:hover,
  .menu .recent .x:hover {
    background: none;
  }
  .recent .nm {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .recent .where {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .menu .recent .x {
    width: auto;
    color: var(--muted);
    padding: 6px 10px;
  }
  .menu .recent .x:hover {
    color: var(--text);
  }
  .recent .empty {
    margin: 8px 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .menu .recent .clear {
    color: var(--muted);
    font-size: 12px;
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
