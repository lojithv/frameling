<script lang="ts">
  // The character creator: remix parts from the library (built-in and part
  // packs) with a live, animated preview. Opens from the inspector ("Open
  // character creator") or the library ("Create a character").
  import { untrack } from "svelte";
  import { app, eng, currentScene, run, toast } from "../lib/store.svelte";
  import { CHARACTER_PRESETS, PALETTE, SKIN_TONES, newCharacter } from "../lib/defaults";
  import { toImageData } from "../lib/thumbs";
  import { loadPackFile } from "../lib/platform";
  import { loadMyCharacters, saveMyCharacter, deleteMyCharacter, type MyCharacter } from "../lib/mycharacters";
  import type { CharacterKind, Item, Look, PartInfo } from "../lib/types";

  // ------------------------------------------------------------ working state
  const target = $derived(app.creator);
  const scene = $derived(currentScene());
  const editing = $derived(target?.itemId ? (scene?.items.find((i) => i.id === target.itemId) ?? null) : null);

  const clone = <T,>(v: T): T => structuredClone($state.snapshot(v)) as T;
  const startLook = (): Look =>
    clone(target?.look ?? (editing?.kind.type === "character" ? (editing.kind as CharacterKind).look : CHARACTER_PRESETS[0].look));

  let look = $state<Look>(untrack(startLook));
  // The creator is re-created for each opening, so reading the target once is intended.
  let name = $state(untrack(() => target?.name ?? editing?.name ?? "New character"));
  let past: Look[] = $state([]);
  let future: Look[] = $state([]);

  /** Change the look as one undo step. */
  function change(next: Look) {
    past = [...past.slice(-80), clone(look)];
    future = [];
    look = next;
  }
  function undo() {
    if (!past.length) return;
    future = [clone(look), ...future];
    look = past[past.length - 1];
    past = past.slice(0, -1);
  }
  function redo() {
    if (!future.length) return;
    past = [...past, clone(look)];
    look = future[0];
    future = future.slice(1);
  }

  const stick = $derived(look.style === "stick");
  let packRev = $state(0); // bumped after loading a part pack
  const catalog = $derived.by((): PartInfo[] => {
    void packRev;
    return JSON.parse(eng().parts_catalog(stick ? "stick" : "cartoon"));
  });
  const selected = $derived.by((): Record<string, string> => JSON.parse(eng().selected_parts(JSON.stringify($state.snapshot(look)))));

  // ------------------------------------------------------------ tabs and parts
  type Tab = { id: string; label: string; icon: string; cats: string[]; colors: string[]; head?: boolean };
  const TABS = $derived<Tab[]>([
    { id: "body", label: "Body", icon: "🧍", cats: ["body"], colors: stick ? ["skin", "top", "shoes"] : ["skin"] },
    { id: "hair", label: "Hair", icon: "💇", cats: ["hair"], colors: ["hair"], head: true },
    ...(stick
      ? [{ id: "face", label: "Face", icon: "🙂", cats: ["facialHair"], colors: ["hair"], head: true }]
      : [{ id: "face", label: "Face", icon: "🙂", cats: ["eyes", "brows", "nose", "facialHair"], colors: ["hair"], head: true }]),
    { id: "clothes", label: "Clothes", icon: "👕", cats: stick ? ["outfit"] : [], colors: stick ? ["accent"] : ["top", "bottom", "shoes"] },
    { id: "wear", label: "Wearables", icon: "🎩", cats: ["headwear", "eyewear", "neckwear", "earrings", "back", "held"], colors: [] },
    { id: "mine", label: "My characters", icon: "⭐", cats: [], colors: [] },
  ]);
  let tab = $state("body");
  const current = $derived(TABS.find((t) => t.id === tab) ?? TABS[0]);
  const CAT_LABEL: Record<string, string> = {
    body: "Body shape", hair: "Hair style", eyes: "Eyes", brows: "Eyebrows", nose: "Nose", facialHair: "Facial hair",
    outfit: "Outfit", headwear: "Headwear", eyewear: "Eyewear", neckwear: "Neckwear", earrings: "Earrings", back: "On the back", held: "Held items",
  };
  const HEAD_CATS = new Set(["hair", "eyes", "brows", "nose", "facialHair", "headwear", "eyewear", "earrings"]);
  let query = $state("");
  const partsIn = (cat: string) =>
    catalog.filter((p) => p.category === cat && (!query || `${p.name} ${p.tags.join(" ")}`.toLowerCase().includes(query.toLowerCase())));

  const lookJson = () => JSON.stringify($state.snapshot(look));
  function withPart(base: Look, id: string, colors: string[] = []): Look {
    return JSON.parse(eng().apply_part(JSON.stringify(base), id, JSON.stringify(colors)));
  }
  function pick(p: PartInfo) {
    try {
      // Keep the colours already chosen for a wearable when re-picking it.
      const kept = (look.wear ?? []).find((w) => w.part === p.id)?.colors ?? [];
      change(withPart($state.snapshot(look) as Look, p.id, kept));
    } catch (e) {
      toast(String((e as Error).message ?? e), "error");
    }
  }

  // ------------------------------------------------------------ colours
  const FIELD_LABEL = $derived<Record<string, string>>({
    skin: stick ? "Head" : "Skin", hair: "Hair", top: stick ? "Lines" : "Top", bottom: "Bottom", shoes: stick ? "Feet" : "Shoes", accent: stick ? "Dress and extras" : "Accessory colour",
  });
  const FIELD_SWATCHES: Record<string, string[]> = {
    skin: SKIN_TONES.concat(["#ffffff"]),
    hair: ["#1b1311", "#2d2d2d", "#3b2a20", "#5a3825", "#8b5a2b", "#d99a3e", "#e8c27a", "#b5452b", "#9aa0a6", "#e9e6df", "#4f7cff", "#f15bb5"],
    top: PALETTE.slice(0, 1).concat(PALETTE.slice(2)),
    bottom: ["#2f3a56", "#454d72", "#1f2233", "#264653", "#6b4630", "#9aa3b5", "#3a405a", "#f1f1ef", "#2a9d8f", "#e76f51"],
    shoes: ["#2a221e", "#1f2233", "#5b3a29", "#ffffff", "#8a8f99", "#e84a5f", "#4f7cff"],
    accent: PALETTE.slice(2),
  };
  function setField(field: string, c: string) {
    change({ ...clone(look), [field]: c } as Look);
  }
  /** Colours for wearables worn in this tab: [part, slot index, current, default]. */
  const wearColors = $derived.by(() => {
    if (tab !== "wear") return [];
    const out: { part: PartInfo; slot: number; value: string }[] = [];
    for (const w of look.wear ?? []) {
      const p = catalog.find((x) => x.id === w.part);
      if (!p) continue;
      p.colors.forEach((def, i) => out.push({ part: p, slot: i, value: w.colors?.[i] ?? def }));
    }
    return out;
  });
  function setWearColor(partId: string, slot: number, c: string) {
    const next = clone(look);
    const w = (next.wear ?? []).find((x) => x.part === partId);
    const p = catalog.find((x) => x.id === partId);
    if (!w || !p) return;
    const cs = [...(w.colors ?? [])];
    while (cs.length < p.colors.length) cs.push(p.colors[cs.length]);
    cs[slot] = c;
    w.colors = cs;
    change(next);
  }

  // ------------------------------------------------------------ style, randomise
  function setStyle(s: "cartoon" | "stick") {
    if ((look.style ?? "cartoon") === s) return;
    const next = clone(look);
    next.style = s;
    if (s === "stick") Object.assign(next, { skin: "#ffffff", top: "#1f2233", bottom: "#1f2233", shoes: "#1f2233" });
    else Object.assign(next, { skin: "#e9b48a", top: "#4f7cff", bottom: "#2f3a56", shoes: "#2a221e", dress: false });
    change(next);
  }
  let locks = $state<Record<string, boolean>>({});
  /** Engine lock names for each tab. */
  const LOCKS: Record<string, string[]> = {
    body: ["body", "skin"], hair: ["hair"], face: ["eyes", "brows", "nose", "facialHair"], clothes: ["outfit", "clothes"],
    wear: ["headwear", "eyewear", "neckwear", "earrings", "back", "held"],
  };
  function randomize() {
    const l = Object.entries(locks).filter(([, on]) => on).flatMap(([t]) => LOCKS[t] ?? []);
    const seed = Math.floor(Math.random() * 2 ** 31);
    change(JSON.parse(eng().randomize_look(lookJson(), seed, JSON.stringify(l))));
  }

  // ------------------------------------------------------------ preview
  let canvas: HTMLCanvasElement | undefined = $state();
  let action = $state("idle");
  let expression = $state<CharacterKind["expression"]>("happy");
  const PREVIEW_ACTIONS = [
    { id: "idle", label: "Idle" }, { id: "talk", label: "Talk" }, { id: "wave", label: "Wave" }, { id: "walk", label: "Walk" }, { id: "cheer", label: "Cheer" },
  ];
  const W = 300, H = 380;
  function previewItem(l: Look, act = action): Item {
    const it = newCharacter(l, name);
    (it.kind as CharacterKind).action = act;
    (it.kind as CharacterKind).expression = expression;
    (it.kind as CharacterKind).autoWalk = false;
    return it;
  }
  $effect(() => {
    const c = canvas;
    if (!c) return;
    const json = JSON.stringify(previewItem($state.snapshot(look) as Look, action));
    let frame = 0;
    let raf = 0;
    let last = 0;
    const ctx = c.getContext("2d")!;
    const tick = (t: number) => {
      raf = requestAnimationFrame(tick);
      if (t - last < 1000 / 24) return;
      last = t;
      try {
        ctx.putImageData(toImageData(eng().render_item_preview(json, W, H, frame), W, H), 0, 0);
      } catch {
        /* engine busy */
      }
      frame = (frame + 1) % 3600;
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  // ------------------------------------------------------------ thumbnails
  // Each option is drawn on the current character, so you see the real result.
  const thumbCache = new Map<string, string>();
  const scratch = typeof document !== "undefined" ? document.createElement("canvas") : null;
  function thumb(p: PartInfo): string {
    let l: Look;
    try {
      l = withPart($state.snapshot(look) as Look, p.id, (look.wear ?? []).find((w) => w.part === p.id)?.colors ?? []);
    } catch {
      return "";
    }
    const head = HEAD_CATS.has(p.category);
    const key = `${head}:${JSON.stringify(l)}`;
    const hit = thumbCache.get(key);
    if (hit) return hit;
    const it = previewItem(l, "idle");
    (it.kind as CharacterKind).expression = "neutral";
    const [w, h] = head ? [180, 300] : [110, 140];
    const px = eng().render_item_preview(JSON.stringify(it), w, h, 0);
    const c = scratch!;
    c.width = 96;
    c.height = 96;
    const ctx = c.getContext("2d")!;
    ctx.clearRect(0, 0, 96, 96);
    const tmp = document.createElement("canvas");
    tmp.width = w;
    tmp.height = h;
    tmp.getContext("2d")!.putImageData(toImageData(px, w, h), 0, 0);
    // Head parts: zoom on the top of the figure.
    if (head) ctx.drawImage(tmp, w * 0.18, h * 0.02, w * 0.64, w * 0.64, 2, 2, 92, 92);
    else ctx.drawImage(tmp, (96 - w * 0.68) / 2, 2, w * 0.68, h * 0.68);
    const url = c.toDataURL();
    if (thumbCache.size > 600) thumbCache.clear();
    thumbCache.set(key, url);
    return url;
  }

  // ------------------------------------------------------------ my characters
  let mine = $state<MyCharacter[]>(loadMyCharacters());
  function saveMine() {
    mine = saveMyCharacter(name.trim() || "Character", clone(look));
    toast(`Saved ${name} to My characters`);
  }
  function loadMine(m: MyCharacter) {
    change(clone(m.look));
    name = m.name;
  }
  function presetThumb(l: Look, key: string) {
    const k = `preset:${key}:${JSON.stringify(l)}`;
    const hit = thumbCache.get(k);
    if (hit) return hit;
    const px = eng().render_item_preview(JSON.stringify(previewItem(l, "idle")), 90, 120, 0);
    const c = document.createElement("canvas");
    c.width = 90;
    c.height = 120;
    c.getContext("2d")!.putImageData(toImageData(px, 90, 120), 0, 0);
    const url = c.toDataURL();
    thumbCache.set(k, url);
    return url;
  }

  // ------------------------------------------------------------ finish
  /** Copy definitions of pack parts this look wears into the project. */
  function partCommands(l: Look) {
    const ids = (l.wear ?? []).map((w) => w.part).filter((id) => !app.project?.parts?.[id]);
    const defs = JSON.parse(eng().part_defs(JSON.stringify(ids))) as unknown[];
    return defs.length ? [{ op: "addParts" as const, parts: defs }] : [];
  }
  function apply() {
    if (!scene) return;
    const l = clone(look);
    if (editing && editing.kind.type === "character") {
      const it = clone(editing);
      (it.kind as CharacterKind).look = l;
      it.name = name.trim() || it.name;
      if (run({ op: "batch", commands: [...partCommands(l), { op: "updateItem", scene: scene.id, item: it }] })) close();
    } else {
      const it = newCharacter(l, name.trim() || "Character");
      if (run({ op: "batch", commands: [...partCommands(l), { op: "addItem", scene: scene.id, item: it }] })) {
        app.selected = it.id;
        close();
      }
    }
  }
  function close() {
    app.creator = null;
  }
  async function loadPack() {
    await loadPackFile();
    packRev++;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "z") {
      e.preventDefault();
      e.stopPropagation();
      if (e.shiftKey) redo();
      else undo();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="scrim" role="presentation" onclick={close}></div>
<div class="creator" role="dialog" aria-modal="true" aria-label="Character creator">
  <header>
    <h2>Character creator</h2>
    <input class="name" bind:value={name} aria-label="Character name" />
    <div class="seg">
      <button class:on={!stick} onclick={() => setStyle("cartoon")}>Cartoon</button>
      <button class:on={stick} onclick={() => setStyle("stick")}>Stick figure</button>
    </div>
    <div class="spacer"></div>
    <button onclick={undo} disabled={!past.length} title="Undo (Ctrl/⌘ Z)">↶</button>
    <button onclick={redo} disabled={!future.length} title="Redo (Ctrl/⌘ Shift Z)">↷</button>
    <button class="close" onclick={close} title="Close (Esc)" aria-label="Close">✕</button>
  </header>

  <div class="main">
    <section class="stagebox">
      <div class="preview" class:paper={stick}>
        <canvas bind:this={canvas} width={W} height={H}></canvas>
      </div>
      <div class="chips">
        {#each PREVIEW_ACTIONS as a}
          <button class:on={action === a.id} onclick={() => (action = a.id)}>{a.label}</button>
        {/each}
      </div>
      <div class="chips small">
        {#each ["neutral", "happy", "sad", "angry", "surprised", "wink"] as ex}
          <button class:on={expression === ex} onclick={() => (expression = ex as CharacterKind["expression"])}>{ex}</button>
        {/each}
      </div>
      <div class="rand">
        <button class="primary-ghost" onclick={randomize} title="Randomise everything that isn't locked">🎲 Randomise</button>
        <div class="locks">
          <span>Keep:</span>
          {#each TABS.filter((t) => LOCKS[t.id]) as t}
            <label class:on={locks[t.id]}><input type="checkbox" bind:checked={locks[t.id]} />{t.label}</label>
          {/each}
        </div>
      </div>
    </section>

    <section class="parts">
      <nav class="tabs" aria-label="Part categories">
        {#each TABS as t}
          <button class:active={tab === t.id} onclick={() => (tab = t.id)}><span>{t.icon}</span>{t.label}</button>
        {/each}
      </nav>

      <div class="scroll">
        {#if tab === "mine"}
          <h4>My characters</h4>
          {#if mine.length === 0}
            <p class="hint">Characters you save appear here, ready to reuse in any project.</p>
          {/if}
          <div class="grid tall">
            {#each mine as m (m.id)}
              <div class="mine">
                <button class="card" onclick={() => loadMine(m)} title="Edit a copy of {m.name}">
                  <img alt="" src={presetThumb(m.look, m.id)} /><span>{m.name}</span>
                </button>
                <button class="del" title="Remove from My characters" aria-label="Remove {m.name}" onclick={() => (mine = deleteMyCharacter(m.id))}>✕</button>
              </div>
            {/each}
          </div>
          <h4>Start from the cast</h4>
          <div class="grid tall">
            {#each CHARACTER_PRESETS as c, i}
              <button class="card" class:paper={c.group === "stick"} onclick={() => { change(clone(c.look)); name = c.name; }} title="Customise a copy of {c.name}">
                <img alt="" src={presetThumb(c.look, `cast${i}`)} /><span>{c.name}</span>
              </button>
            {/each}
          </div>
        {:else}
          {#if tab === "wear"}
            <input class="search" placeholder="Search wearables… (try winter, music, chef)" bind:value={query} />
          {/if}
          {#each current.cats as cat}
            {@const list = partsIn(cat)}
            {#if list.length}
              <h4>{CAT_LABEL[cat] ?? cat}</h4>
              <div class="grid">
                {#each list as p (p.id)}
                  <button class="card" class:on={selected[p.category] === p.id} class:paper={stick} onclick={() => pick(p)} title={p.tags.length ? `${p.name}: ${p.tags.join(", ")}` : p.name}>
                    <img alt="" src={thumb(p)} />
                    <span>{p.name}</span>
                    {#if p.pack !== "builtin" && p.pack !== "parts"}<em class="pack">{p.pack}</em>{/if}
                  </button>
                {/each}
              </div>
            {/if}
          {/each}
          {#if current.colors.length}
            <h4>Colours</h4>
            {#each current.colors as f}
              <div class="field">
                <span>{FIELD_LABEL[f]}</span>
                <div class="swatches">
                  {#each FIELD_SWATCHES[f] as c}
                    <button class="sw" class:on={(look as unknown as Record<string, string>)[f]?.slice(0, 7).toLowerCase() === c} style:background={c} title={c} aria-label={c} onclick={() => setField(f, c)}></button>
                  {/each}
                  <label class="sw custom" title="Custom colour"><input type="color" value={(look as unknown as Record<string, string>)[f]?.slice(0, 7)} onchange={(e) => setField(f, (e.target as HTMLInputElement).value)} /></label>
                </div>
              </div>
            {/each}
          {/if}
          {#if tab === "face" && !stick}
            <p class="hint">Expressions, blinking and lip sync work with every face part. Use the buttons under the preview to try them.</p>
          {/if}
          {#if wearColors.length}
            <h4>Wearable colours</h4>
            {#each wearColors as wc (wc.part.id + wc.slot)}
              <div class="field">
                <span>{wc.part.name}{wc.part.colors.length > 1 ? ` ${wc.slot + 1}` : ""}</span>
                <div class="swatches">
                  {#each [wc.part.colors[wc.slot], ...PALETTE.slice(0, 1), ...PALETTE.slice(2, 12)] as c, i (i)}
                    <button class="sw" class:on={wc.value.slice(0, 7).toLowerCase() === c.toLowerCase()} style:background={c} title={c} aria-label={c} onclick={() => setWearColor(wc.part.id, wc.slot, c)}></button>
                  {/each}
                  <label class="sw custom" title="Custom colour"><input type="color" value={wc.value.slice(0, 7)} onchange={(e) => setWearColor(wc.part.id, wc.slot, (e.target as HTMLInputElement).value)} /></label>
                </div>
              </div>
            {/each}
          {/if}
          {#if tab === "wear"}
            <button class="ghost wide" onclick={loadPack}>＋ Load a part pack…</button>
            <p class="hint">Part packs add your own hats, glasses and props for hands. See "Custom parts" in docs/CHARACTERS.md.</p>
          {/if}
        {/if}
      </div>
    </section>
  </div>

  <footer>
    <button onclick={saveMine}>⭐ Save to My characters</button>
    <div class="spacer"></div>
    <button onclick={close}>Cancel</button>
    <button class="primary" onclick={apply} disabled={!scene}>{editing ? `Apply to ${editing.name}` : "Add to scene"}</button>
  </footer>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 18, 0.6);
    z-index: 40;
  }
  .creator {
    position: fixed;
    inset: 4vh 4vw;
    max-width: 1180px;
    margin: 0 auto;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-radius: 14px;
    z-index: 41;
    display: flex;
    flex-direction: column;
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  footer {
    border-bottom: none;
    border-top: 1px solid var(--line);
  }
  h2 {
    margin: 0 8px 0 0;
    font-size: 16px;
  }
  .name {
    width: 200px;
  }
  .spacer {
    flex: 1;
  }
  .close {
    width: 32px;
  }
  .seg {
    display: flex;
    background: var(--bg-2);
    border-radius: 8px;
    padding: 2px;
  }
  .seg button {
    border: none;
    background: none;
    padding: 5px 12px;
  }
  .seg button.on {
    background: var(--accent);
    color: #fff;
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .stagebox {
    width: 340px;
    flex-shrink: 0;
    border-right: 1px solid var(--line);
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
  }
  .preview {
    border-radius: 12px;
    background: radial-gradient(circle at 50% 35%, #eef1fb, #cdd3e6);
    display: grid;
    place-items: center;
  }
  .preview.paper {
    background: #fffdf5;
  }
  .preview canvas {
    width: 300px;
    height: 380px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chips button {
    padding: 4px 9px;
    font-size: 12px;
    border-radius: 999px;
  }
  .chips.small button {
    font-size: 11px;
    padding: 3px 8px;
    text-transform: capitalize;
  }
  .chips button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .rand {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 4px;
  }
  .primary-ghost {
    border-color: var(--accent);
    color: var(--accent-2);
    font-weight: 600;
  }
  .locks {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
    font-size: 11px;
    color: var(--muted);
  }
  .locks label {
    display: flex;
    gap: 3px;
    align-items: center;
    padding: 2px 6px;
    border: 1px solid var(--line);
    border-radius: 999px;
    cursor: pointer;
  }
  .locks label.on {
    border-color: var(--accent);
    color: var(--text);
  }
  .locks input {
    margin: 0;
  }
  .parts {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 8px 12px 0;
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
  }
  .tabs button {
    border: none;
    border-radius: 8px 8px 0 0;
    background: none;
    padding: 8px 12px;
    display: flex;
    gap: 6px;
    align-items: center;
    white-space: nowrap;
    color: var(--muted);
  }
  .tabs button.active {
    background: var(--bg-2);
    color: var(--text);
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 6px 16px 16px;
  }
  h4 {
    margin: 14px 0 8px;
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 8px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 6px 4px 6px;
    background: var(--bg-2);
    border: 2px solid transparent;
    border-radius: 10px;
    font-size: 11px;
  }
  .card img {
    width: 88px;
    height: 88px;
    border-radius: 8px;
    background: radial-gradient(circle at 50% 40%, #eef1fb, #d3d8ea);
  }
  .card.paper img {
    background: #fffdf5;
  }
  .grid.tall .card img {
    height: 110px;
    width: 82px;
  }
  .card.on {
    border-color: var(--accent);
    background: rgba(91, 108, 255, 0.14);
  }
  .card .pack {
    position: absolute;
    top: 8px;
    left: 8px;
    font-style: normal;
    font-size: 9px;
    background: var(--accent);
    color: #fff;
    padding: 1px 4px;
    border-radius: 4px;
  }
  .mine {
    position: relative;
  }
  .mine .card {
    width: 100%;
  }
  .mine .del {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 22px;
    height: 22px;
    padding: 0;
    font-size: 11px;
    border-radius: 50%;
    opacity: 0.7;
  }
  .search {
    width: 100%;
    margin-top: 12px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 8px 0;
  }
  .field > span {
    width: 120px;
    flex-shrink: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .sw {
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.15);
  }
  .sw.on {
    border-color: #fff;
    box-shadow: 0 0 0 2px var(--accent);
  }
  .sw.custom {
    position: relative;
    overflow: hidden;
    background: conic-gradient(red, yellow, lime, cyan, blue, magenta, red);
    cursor: pointer;
  }
  .sw.custom input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .hint {
    font-size: 12px;
    color: var(--muted);
    line-height: 1.45;
  }
  .ghost.wide {
    width: 100%;
    margin-top: 16px;
  }
  footer .primary {
    padding: 8px 18px;
  }
</style>
