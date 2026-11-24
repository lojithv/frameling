<script lang="ts">
  import { app, currentScene, run, toast } from "../lib/store.svelte";
  import { CHARACTER_PRESETS, TEXT_PRESETS, newCharacter, newProp, newTextPreset, uid } from "../lib/defaults";
  import { itemThumb, backgroundThumb } from "../lib/thumbs";
  import { importAudio, previewSound, stopAudio } from "../lib/audio";
  import { eng } from "../lib/store.svelte";
  import type { Music, SoundInfo } from "../lib/types";
  import { loadPackFile } from "../lib/platform";
  import { loadMyCharacters } from "../lib/mycharacters";
  import type { CatalogEntry, Item, Scene } from "../lib/types";

  const tabs = [
    { id: "characters", label: "Characters", icon: "🧑" },
    { id: "props", label: "Props", icon: "🪴" },
    { id: "backgrounds", label: "Scenes", icon: "🏞️" },
    { id: "text", label: "Text", icon: "💬" },
    { id: "sound", label: "Sound", icon: "🎵" },
  ] as const;
  let tab = $state<(typeof tabs)[number]["id"]>("characters");
  let query = $state("");

  const scene = $derived(currentScene());
  const catalog = $derived(app.catalog);
  const propsByCat = $derived.by(() => {
    const groups = new Map<string, CatalogEntry[]>();
    for (const p of catalog?.props ?? []) {
      if (query && !p.name.toLowerCase().includes(query.toLowerCase())) continue;
      const c = p.category || "Other";
      if (!groups.has(c)) groups.set(c, []);
      groups.get(c)!.push(p);
    }
    return [...groups.entries()].sort();
  });

  function add(item: Item) {
    if (!scene) return;
    if (run({ op: "addItem", scene: scene.id, item })) app.selected = item.id;
  }

  function dragData(e: DragEvent, data: object) {
    e.dataTransfer?.setData("application/x-frameling", JSON.stringify(data));
    e.dataTransfer!.effectAllowed = "copy";
  }

  function setBackground(id: string) {
    if (!scene) return;
    run({ op: "updateScene", scene: { ...$state.snapshot(scene), background: { preset: id, colors: [] } } as Scene });
  }

  let importing = $state(false);
  async function onAudioFile(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file || !scene) return;
    importing = true;
    try {
      const media = await importAudio(file);
      const speaker = scene.items.find((i) => i.id === app.selected && i.kind.type === "character")?.id ?? null;
      const clip = { id: uid("clip"), media: media.id, start: app.frame, volume: 1, speaker, trimStart: 0, length: null };
      run({ op: "batch", commands: [{ op: "addMedia", media }, { op: "addAudio", scene: scene.id, clip }] });
      toast(speaker ? "Voice added. The selected character will lip-sync to it." : "Sound added to this scene");
    } catch (err) {
      toast(`Could not read that audio file (${(err as Error).message})`, "error");
    } finally {
      importing = false;
    }
  }

  function addExisting(mediaId: string) {
    if (!scene) return;
    const speaker = scene.items.find((i) => i.id === app.selected && i.kind.type === "character")?.id ?? null;
    run({ op: "addAudio", scene: scene.id, clip: { id: uid("clip"), media: mediaId, start: app.frame, volume: 1, speaker, trimStart: 0, length: null } });
  }

  const media = $derived(Object.values(app.project?.media ?? {}));

  // ---------------------------------------------------- built-in sounds
  const sounds: SoundInfo[] = $derived(app.ready ? JSON.parse(eng().sound_catalog()) : []);
  const effectsByCat = $derived.by(() => {
    const groups = new Map<string, SoundInfo[]>();
    for (const s of sounds.filter((s) => s.kind === "effect")) {
      if (!groups.has(s.category)) groups.set(s.category, []);
      groups.get(s.category)!.push(s);
    }
    return [...groups.entries()];
  });
  const loops = $derived(sounds.filter((s) => s.kind === "music"));
  let previewing = $state<string | null>(null);
  function preview(id: string) {
    if (previewing === id) {
      stopAudio();
      previewing = null;
      return;
    }
    previewing = id;
    previewSound(id);
  }
  function addEffect(s: SoundInfo) {
    if (!scene) return;
    const clip = { id: uid("clip"), media: `sound-${s.id}`, start: app.frame, volume: 0.8, speaker: null, trimStart: 0, length: null };
    if (run({ op: "batch", commands: [{ op: "addBuiltinMedia", sound: s.id }, { op: "addAudio", scene: scene.id, clip }] })) toast(`${s.name} added at the playhead`);
  }
  /** Use a sound (built-in id or project media) as the movie's background music. */
  function useAsMusic(mediaId: string, builtin?: string) {
    const old = app.project?.music;
    const music: Music = { media: mediaId, volume: old?.volume ?? 0.5, duck: old?.duck ?? 0.3, fadeIn: old?.fadeIn ?? 15, fadeOut: old?.fadeOut ?? 45, looped: old?.looped ?? true, start: 0, trimStart: 0 };
    const cmds: import("../lib/types").Command[] = builtin ? [{ op: "addBuiltinMedia", sound: builtin }] : [];
    cmds.push({ op: "setMusic", music });
    if (run({ op: "batch", commands: cmds })) toast("Background music set. It gets quieter while characters talk (scene inspector → Music).");
  }
  // Re-read after the creator closes (it may have saved one).
  const myChars = $derived.by(() => {
    void app.creator;
    return loadMyCharacters();
  });
</script>

<aside class="panel library">
  <nav class="tabs" aria-label="Library">
    {#each tabs as t}
      <button class:active={tab === t.id} onclick={() => (tab = t.id)} title={t.label}>
        <span class="ico">{t.icon}</span><span class="lbl">{t.label}</span>
      </button>
    {/each}
  </nav>

  <div class="body">
    {#if tab === "characters"}
      <button class="create" onclick={() => (app.creator = { itemId: null })}>✎ Create a character</button>
      <p class="hint">Click to add, or drag onto the stage. Restyle anyone in the inspector or the creator.</p>
      {#if myChars.length}
        <h4>My characters</h4>
        <div class="grid">
          {#each myChars as m (m.id)}
            <button class="card" class:paper={m.look.style === "stick"} onclick={() => add(newCharacter(m.look, m.name))} title="Add {m.name}">
              <img alt="" src={itemThumb(`my-${m.id}-${m.savedAt}`, newCharacter(m.look, m.name), 120, 140)} />
              <span>{m.name}</span>
            </button>
          {/each}
        </div>
      {/if}
      {#each [["Cartoon", false], ["Stick figures", true]] as [label, stick]}
      <h4>{label}</h4>
      <div class="grid">
        {#each CHARACTER_PRESETS as c, i}
          {#if (c.group === "stick") === stick}
          <button
            class="card"
            class:paper={stick}
            draggable="true"
            ondragstart={(e) => dragData(e, { kind: "character", index: i })}
            onclick={() => add(newCharacter(c.look, c.name))}
          >
            <img alt="" src={itemThumb(`char-${i}`, newCharacter(c.look, c.name), 120, 140)} />
            <span>{c.name}</span>
          </button>
          {/if}
        {/each}
      </div>
      {/each}
    {:else if tab === "props"}
      <input class="search" placeholder="Search props…" bind:value={query} />
      {#each propsByCat as [cat, list]}
        <h4>{cat}</h4>
        <div class="grid">
          {#each list as p}
            <button
              class="card"
              class:paper={cat === "Stick"}
              draggable="true"
              ondragstart={(e) => dragData(e, { kind: "prop", id: p.id, name: p.name })}
              onclick={() => add(newProp(p.id, p.name))}
              title={p.pack !== "core" ? `${p.name} (from ${p.pack})` : p.name}
            >
              <img alt="" src={itemThumb(`prop-${p.id}-${p.pack}`, newProp(p.id, p.name), 96, 80)} />
              <span>{p.name}</span>
            </button>
          {/each}
        </div>
      {/each}
      <button class="ghost wide" onclick={loadPackFile}>＋ Load asset pack…</button>
    {:else if tab === "backgrounds"}
      <p class="hint">Sets the background of the current scene.</p>
      <div class="grid bgs">
        {#each catalog?.backgrounds ?? [] as b}
          <button
            class="card bg"
            class:active={scene?.background.preset === b.id}
            draggable="true"
            ondragstart={(e) => dragData(e, { kind: "background", id: b.id })}
            onclick={() => setBackground(b.id)}
          >
            <img alt="" src={backgroundThumb(b.id)} />
            <span>{b.name}</span>
          </button>
        {/each}
      </div>
    {:else if tab === "text"}
      <div class="list">
        {#each TEXT_PRESETS as t}
          <button
            class="row"
            draggable="true"
            ondragstart={(e) => dragData(e, { kind: "text", preset: t.id })}
            onclick={() => add(newTextPreset(t))}
          >
            <img alt="" src={itemThumb(`text-${t.id}`, newTextPreset(t), 120, 56, 150)} />
            <span>{t.label}</span>
          </button>
        {/each}
      </div>
    {:else if tab === "sound"}
      <p class="hint">
        Import a voice line or music. If a character is selected, it lip-syncs to the sound automatically.
      </p>
      <label class="primary wide file">
        {importing ? "Reading audio…" : "＋ Import audio (MP3, WAV, OGG…)"}
        <input type="file" accept="audio/*" onchange={onAudioFile} disabled={importing} />
      </label>
      {#if media.length}
        <h4>In this project</h4>
        <div class="list">
          {#each media as m}
            <div class="row sound">
              <button class="grow plain" onclick={() => addExisting(m.id)} title="Add to this scene at the playhead">
                <span class="note">♪</span>
                <span class="grow">{m.name}</span>
                <span class="dim">{(m.durationFrames / (app.project?.fps ?? 30)).toFixed(1)}s</span>
              </button>
              {#if m.source?.kind !== "tts"}
                <button class="mini" class:on={app.project?.music?.media === m.id} title="Use as background music for the whole movie" onclick={() => useAsMusic(m.id)}>🎵</button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
      <h4>Music</h4>
      <p class="hint">Original loops made by Frameling, free to use anywhere. They play under the whole movie and get quieter while characters talk.</p>
      <div class="list">
        {#each loops as s (s.id)}
          <div class="row sound" class:current={app.project?.music?.media === `sound-${s.id}`}>
            <button class="mini" title={previewing === s.id ? "Stop" : "Listen"} onclick={() => preview(s.id)}>{previewing === s.id ? "■" : "▶"}</button>
            <span class="grow">{s.name}<span class="dim"> · {s.category}</span></span>
            <button class="mini use" onclick={() => useAsMusic(`sound-${s.id}`, s.id)}>{app.project?.music?.media === `sound-${s.id}` ? "Playing" : "Use"}</button>
          </div>
        {/each}
      </div>
      <h4>Sound effects</h4>
      <p class="hint">Click a name to add it at the playhead; ▶ listens first.</p>
      {#each effectsByCat as [cat, list]}
        <h5>{cat}</h5>
        <div class="sfx">
          {#each list as s (s.id)}
            <div class="chip">
              <button class="mini" title={previewing === s.id ? "Stop" : "Listen"} onclick={() => preview(s.id)}>{previewing === s.id ? "■" : "▶"}</button>
              <button class="plain" onclick={() => addEffect(s)} title="Add at the playhead">{s.name}</button>
            </div>
          {/each}
        </div>
      {/each}
    {/if}
  </div>
</aside>

<style>
  .library {
    width: 268px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--line);
  }
  .tabs {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 10px 0 8px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--muted);
    font-size: 10.5px;
    border-radius: 0;
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .tabs .ico {
    font-size: 17px;
  }
  .body {
    overflow: auto;
    padding: 12px;
    flex: 1;
  }
  .hint {
    color: var(--muted);
    font-size: 12px;
    margin: 0 0 10px;
    line-height: 1.45;
  }
  h4 {
    margin: 14px 0 8px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 6px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 10px;
    font-size: 12px;
    color: var(--text);
    transition: transform 0.12s, border-color 0.12s, background 0.12s;
  }
  .card:hover {
    border-color: var(--accent);
    background: var(--bg-3);
    transform: translateY(-1px);
  }
  .card img {
    width: 100%;
    max-height: 96px;
    object-fit: contain;
    pointer-events: none;
  }
  /* Ink-drawn stick art needs a light backdrop to show on the dark panel. */
  .card.paper img {
    background: #f6f4ee;
    border-radius: 8px;
  }
  .bgs {
    grid-template-columns: 1fr;
  }
  .card.bg {
    padding: 6px;
  }
  .card.bg img {
    border-radius: 6px;
    max-height: none;
  }
  .card.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 10px;
    color: var(--text);
    font-size: 13px;
    text-align: left;
  }
  .row:hover {
    border-color: var(--accent);
  }
  .row img {
    width: 84px;
    height: 40px;
    object-fit: contain;
  }
  .row.sound .note {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: rgba(255, 207, 90, 0.15);
    color: #ffcf5a;
  }
  .grow {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dim {
    color: var(--muted);
    font-size: 12px;
  }
  .row .plain,
  .chip .plain {
    background: none;
    border: none;
    padding: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    color: inherit;
    font: inherit;
    min-width: 0;
  }
  .row.current {
    border-color: var(--accent);
  }
  .mini {
    padding: 2px 7px;
    font-size: 11px;
    border-radius: 6px;
    flex-shrink: 0;
  }
  .mini.on,
  .mini.use:hover {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  h5 {
    margin: 10px 0 5px;
    font-size: 11px;
    color: var(--muted);
    font-weight: 600;
  }
  .sfx {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px 3px 3px;
    border: 1px solid var(--line);
    border-radius: 999px;
    font-size: 12px;
    background: var(--bg-2);
  }
  .search {
    width: 100%;
    margin-bottom: 4px;
  }
  .wide {
    width: 100%;
    margin-top: 12px;
  }
  .file {
    display: block;
    text-align: center;
    cursor: pointer;
  }
  .file input {
    display: none;
  }
  .create {
    width: 100%;
    margin-bottom: 8px;
    padding: 9px;
    font-weight: 600;
    border-color: var(--accent);
    color: var(--accent-2);
  }
</style>
