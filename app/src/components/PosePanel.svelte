<script lang="ts">
  // The pose library for the selected character: hold a pose for a while
  // (blending in and out), key it at the playhead, or chain poses into a new
  // animation. Thumbnails show each pose on this character.
  import { app, eng, currentScene, editItem, run, toast, secs, frames, fmtTime } from "../lib/store.svelte";
  import { toImageData } from "../lib/thumbs";
  import type { CharacterKind, Item, PoseInfo, PoseMask, PoseStrip } from "../lib/types";

  let { item }: { item: Item } = $props();
  const ch = $derived(item.kind as CharacterKind);
  const scene = $derived(currentScene());

  type Mode = "hold" | "key" | "chain";
  let mode = $state<Mode>("hold");
  let mask = $state<PoseMask>("full");
  let mirror = $state(false);
  let holdSecs = $state(2);
  let blendSecs = $state(0.3);
  let category = $state("All");
  let query = $state("");

  const allPoses = $derived.by((): PoseInfo[] => {
    const saved = Object.values(app.project?.poses ?? {}).map((p) => ({ id: p.id, name: p.name, category: p.category || "Saved", tags: p.tags ?? [] }));
    const lib = (app.catalog?.poses ?? []).filter((p) => !app.project?.poses?.[p.id]);
    return [...saved, ...lib];
  });
  const categories = $derived(["All", ...new Set(allPoses.map((p) => p.category))]);
  const shown = $derived(
    allPoses.filter(
      (p) => (category === "All" || p.category === category) && (!query || `${p.name} ${p.tags.join(" ")}`.toLowerCase().includes(query.toLowerCase())),
    ),
  );

  // ------------------------------------------------------------ thumbnails
  const cache = new Map<string, string>();
  function thumb(p: PoseInfo): string {
    const look = JSON.stringify(ch.look);
    const key = `${p.id}|${mirror}|${look}|${app.project?.poses?.[p.id] ? JSON.stringify(app.project.poses[p.id].pose) : ""}`;
    const hit = cache.get(key);
    if (hit) return hit;
    const strip: PoseStrip = { pose: p.id, start: 0, end: 10, blendIn: 0, blendOut: 0, mask: "full", mirror, breathe: false };
    const it = { id: "pose-thumb", name: "", kind: { ...$state.snapshot(ch), action: "idle", autoWalk: false, poses: [strip] } };
    const [w, h] = [84, 104];
    try {
      const px = eng().render_item_preview(JSON.stringify(it), w, h, 1);
      const c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      c.getContext("2d")!.putImageData(toImageData(px, w, h), 0, 0);
      const url = c.toDataURL();
      if (cache.size > 300) cache.clear();
      cache.set(key, url);
      return url;
    } catch {
      return "";
    }
  }

  // ------------------------------------------------------------ use a pose
  let chain = $state<{ pose: string; name: string; hold: number; mirror: boolean }[]>([]);
  let chainGap = $state(0.4);
  let chainLoop = $state(true);
  let chainName = $state("");

  function edit(fn: (c: CharacterKind) => void) {
    const it = structuredClone($state.snapshot(item)) as Item;
    fn(it.kind as CharacterKind);
    editItem(it);
  }

  function use(p: PoseInfo) {
    if (!scene) return;
    if (mode === "hold") {
      const start = app.frame;
      const end = Math.min(scene.duration, start + Math.max(1, frames(holdSecs)));
      const blend = Math.min(frames(blendSecs), Math.floor((end - start) / 2));
      const strip: PoseStrip = { pose: p.id, start, end, blendIn: blend, blendOut: blend, mask, mirror, breathe: true };
      edit((c) => (c.poses = [...(c.poses ?? []), strip]));
      toast(`${item.name} holds "${p.name}" from ${fmtTime(start)} to ${fmtTime(end)}`);
    } else if (mode === "key") {
      const keys: [string, number][] = JSON.parse(eng().pose_keys(p.id, mask, mirror));
      run({
        op: "batch",
        commands: keys.map(([path, value]) => ({ op: "setKeys" as const, scene: scene.id, item: item.id, path, keys: [{ frame: app.frame, value, interp: "easeInOut" as const }] })),
      });
      toast(`Keyed "${p.name}" at ${fmtTime(app.frame)}. Key another pose later to animate between them.`);
    } else {
      chain = [...chain, { pose: p.id, name: p.name, hold: 0.4, mirror }];
    }
  }

  function removeStrip(i: number) {
    edit((c) => (c.poses = (c.poses ?? []).filter((_, j) => j !== i)));
  }
  function setStrip(i: number, patch: Partial<PoseStrip>) {
    edit((c) => {
      const s = { ...(c.poses ?? [])[i], ...patch };
      s.end = Math.max(s.start + 1, s.end);
      c.poses = (c.poses ?? []).map((x, j) => (j === i ? s : x));
    });
  }
  const poseName = (id: string) => allPoses.find((p) => p.id === id)?.name ?? id;
  const MASKS: { id: PoseMask; label: string }[] = [
    { id: "full", label: "Whole body" }, { id: "upper", label: "Upper body" }, { id: "lower", label: "Legs" }, { id: "armL", label: "Left arm" }, { id: "armR", label: "Right arm" },
  ];

  function makeAnimation() {
    if (!scene || chain.length === 0) return;
    const name = chainName.trim() || chain.map((c) => c.name).join(" → ");
    const id = `anim-${Date.now().toString(36)}`;
    try {
      const action = JSON.parse(eng().chain_poses(id, name, JSON.stringify(chain.map(({ pose, hold, mirror }) => ({ pose, hold, mirror }))), chainGap, chainLoop));
      const it = structuredClone($state.snapshot(item)) as Item;
      (it.kind as CharacterKind).action = id;
      if (run({ op: "batch", commands: [{ op: "addAction", action }, { op: "updateItem", scene: scene.id, item: it }] })) {
        toast(`Made the animation "${name}"; ${item.name} plays it. Every character can use it from Action.`);
        chain = [];
        chainName = "";
      }
    } catch (e) {
      toast(String((e as Error).message ?? e), "error");
    }
  }

  function saveCurrent() {
    if (!scene) return;
    const json = eng().character_pose(app.sceneIndex, item.id, app.frame);
    if (!json) return;
    const name = prompt("Name this pose", `${item.name} at ${fmtTime(app.frame)}`);
    if (!name) return;
    const id = `pose-${Date.now().toString(36)}`;
    if (run({ op: "addPose", pose: { id, name, category: "Saved", tags: [], pose: JSON.parse(json) } })) toast(`Saved "${name}" to this project's poses`);
  }
</script>

<section>
  <h3>Poses</h3>
  <div class="seg">
    <button class:on={mode === "hold"} onclick={() => (mode = "hold")} title="Hold the pose from the playhead, blending in and out">Hold</button>
    <button class:on={mode === "key"} onclick={() => (mode = "key")} title="Write the pose as keyframes at the playhead">Key</button>
    <button class:on={mode === "chain"} onclick={() => (mode = "chain")} title="Pick poses in order to make a new animation">Make animation</button>
  </div>
  {#if mode !== "chain"}
    <div class="row">
      <select bind:value={mask} aria-label="Which joints the pose moves">
        {#each MASKS as m}<option value={m.id}>{m.label}</option>{/each}
      </select>
      <label class="check"><input type="checkbox" bind:checked={mirror} /> Mirror</label>
    </div>
  {/if}
  {#if mode === "hold"}
    <div class="row nums">
      <label>Hold <input type="number" min="0.1" step="0.1" bind:value={holdSecs} /> s</label>
      <label>Blend <input type="number" min="0" step="0.05" bind:value={blendSecs} /> s</label>
    </div>
    <p class="hint">Click a pose to hold it from the playhead ({fmtTime(app.frame)}). Upper body plays over walking and talking.</p>
  {:else if mode === "key"}
    <p class="hint">Click a pose to key it at the playhead. Keys replace the action on those joints for the whole scene, so key several poses to animate between them.</p>
  {:else}
    <p class="hint">Click poses in order. Each holds for its time, then moves to the next.</p>
    {#if chain.length}
      <ol class="chain">
        {#each chain as c, i (i)}
          <li>
            <span>{c.name}{c.mirror ? " (mirrored)" : ""}</span>
            <input type="number" min="0" step="0.1" bind:value={c.hold} aria-label="Hold seconds" title="Hold (seconds)" />s
            <button class="x" onclick={() => (chain = chain.filter((_, j) => j !== i))} aria-label="Remove">✕</button>
          </li>
        {/each}
      </ol>
      <div class="row nums">
        <label>Move <input type="number" min="0.05" step="0.05" bind:value={chainGap} /> s</label>
        <label class="check"><input type="checkbox" bind:checked={chainLoop} /> Loop</label>
      </div>
      <input class="name" placeholder="Animation name" bind:value={chainName} />
      <button class="primary wide" onclick={makeAnimation}>Make animation from {chain.length} pose{chain.length === 1 ? "" : "s"}</button>
    {/if}
    <label class="check"><input type="checkbox" bind:checked={mirror} /> Add poses mirrored</label>
  {/if}

  <div class="filters">
    <input class="search" placeholder="Search poses…" bind:value={query} />
    <div class="cats">
      {#each categories as c}<button class:on={category === c} onclick={() => (category = c)}>{c}</button>{/each}
    </div>
  </div>
  <div class="grid">
    {#each shown as p (p.id)}
      <button class="pose" onclick={() => use(p)} title={p.tags.length ? `${p.name}: ${p.tags.join(", ")}` : p.name}>
        <img alt="" src={thumb(p)} />
        <span>{p.name}</span>
      </button>
    {/each}
  </div>
  <button class="link" onclick={saveCurrent}>＋ Save this character's current pose</button>

  {#if (ch.poses ?? []).length}
    <h4>Held poses</h4>
    <ul class="strips">
      {#each ch.poses ?? [] as s, i (i)}
        <li>
          <span class="nm">{poseName(s.pose)}{s.mirror ? " ↔" : ""}</span>
          <span class="tm">{fmtTime(s.start)}–{fmtTime(s.end)}</span>
          <select value={s.mask} onchange={(e) => setStrip(i, { mask: (e.target as HTMLSelectElement).value as PoseMask })} aria-label="Joints">
            {#each MASKS as m}<option value={m.id}>{m.label}</option>{/each}
          </select>
          <button class="x" title="Start at the playhead" onclick={() => setStrip(i, { start: app.frame, end: app.frame + (s.end - s.start) })}>⤓</button>
          <button class="x" title="Remove" aria-label="Remove held pose" onclick={() => removeStrip(i)}>✕</button>
          <label class="bl">Blend <input type="number" min="0" step="0.05" value={secs(s.blendIn).toFixed(2)} onchange={(e) => { const b = frames(+(e.target as HTMLInputElement).value); setStrip(i, { blendIn: b, blendOut: b }); }} /> s</label>
          <label class="bl">Length <input type="number" min="0.1" step="0.1" value={secs(s.end - s.start).toFixed(1)} onchange={(e) => setStrip(i, { end: s.start + Math.max(1, frames(+(e.target as HTMLInputElement).value)) })} /> s</label>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .seg {
    display: flex;
    background: var(--bg-2);
    border-radius: 8px;
    padding: 2px;
    margin-bottom: 8px;
  }
  .seg button {
    flex: 1;
    border: none;
    background: none;
    padding: 5px 4px;
    font-size: 12px;
  }
  .seg button.on {
    background: var(--accent);
    color: #fff;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 6px;
  }
  .row select {
    flex: 1;
  }
  .nums label {
    font-size: 12px;
    color: var(--muted);
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .nums input,
  .chain input,
  .bl input {
    width: 52px;
  }
  .check {
    font-size: 12px;
    display: flex;
    gap: 5px;
    align-items: center;
  }
  .hint {
    font-size: 11.5px;
    color: var(--muted);
    line-height: 1.4;
    margin: 4px 0 8px;
  }
  .filters .search {
    width: 100%;
  }
  .cats {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin: 6px 0;
  }
  .cats button {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 999px;
  }
  .cats button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(74px, 1fr));
    gap: 6px;
    max-height: 360px;
    overflow-y: auto;
  }
  .pose {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 4px 2px;
    font-size: 10.5px;
    background: var(--bg-2);
    border: 1px solid transparent;
    border-radius: 8px;
    line-height: 1.2;
  }
  .pose:hover {
    border-color: var(--accent);
  }
  .pose img {
    width: 66px;
    height: 82px;
    background: radial-gradient(circle at 50% 40%, #eef1fb, #d3d8ea);
    border-radius: 6px;
    margin-bottom: 3px;
  }
  .link {
    background: none;
    border: none;
    color: var(--accent-2);
    font-size: 12px;
    padding: 8px 0 0;
  }
  h4 {
    margin: 12px 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .strips,
  .chain {
    list-style: none;
    padding: 0;
    margin: 0 0 8px;
  }
  .strips li {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 6px;
    align-items: center;
    padding: 6px;
    background: var(--bg-2);
    border-radius: 7px;
    margin-bottom: 5px;
    font-size: 12px;
  }
  .strips .nm {
    font-weight: 600;
    flex: 1;
  }
  .strips .tm {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .strips select {
    font-size: 11px;
  }
  .bl {
    font-size: 11px;
    color: var(--muted);
    display: flex;
    gap: 3px;
    align-items: center;
  }
  .x {
    width: 22px;
    height: 22px;
    padding: 0;
    font-size: 11px;
  }
  .chain li {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
    padding: 3px 0;
    counter-increment: step;
  }
  .chain li span {
    flex: 1;
  }
  .chain li::before {
    content: counter(step) ".";
    color: var(--muted);
    width: 16px;
  }
  .name {
    width: 100%;
    margin: 4px 0 6px;
  }
  .wide {
    width: 100%;
  }
</style>
