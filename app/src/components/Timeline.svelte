<script lang="ts">
  import { app, currentScene, run, updateItem, endGesture, fmtTime, secs, frames } from "../lib/store.svelte";
  import { newScene, uid } from "../lib/defaults";
  import { sceneThumb } from "../lib/thumbs";
  import { play, stop } from "../lib/playback";
  import type { Item, Scene, AudioClip } from "../lib/types";

  const project = $derived(app.project);
  const scene = $derived(currentScene());
  const snap = <T,>(v: T): T => structuredClone($state.snapshot(v)) as T;

  // Scene thumbnails refresh lazily when the document changes.
  let thumbs = $state<string[]>([]);
  let thumbTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void app.rev;
    const n = project?.scenes.length ?? 0;
    clearTimeout(thumbTimer);
    thumbTimer = setTimeout(() => (thumbs = Array.from({ length: n }, (_, i) => sceneThumb(i, app.rev))), 250);
  });

  function selectScene(i: number) {
    stop();
    app.sceneIndex = i;
    app.frame = 0;
    app.selected = null;
  }
  function addScene() {
    const s = newScene((project?.scenes.length ?? 0) + 1, scene?.background.preset ?? "studio");
    s.transition = { kind: "fade", duration: 15 };
    if (run({ op: "addScene", scene: s, index: app.sceneIndex + 1 })) selectScene(app.sceneIndex + 1);
  }
  function duplicateScene(i: number) {
    const src = snap(project!.scenes[i]);
    const idMap = new Map<string, string>();
    const copy: Scene = {
      ...src,
      id: uid("scene"),
      name: `${src.name} copy`,
      items: src.items.map((it) => {
        const id = uid(it.kind.type);
        idMap.set(it.id, id);
        return { ...it, id };
      }),
    };
    copy.audio = src.audio.map((c) => ({ ...c, id: uid("clip"), speaker: c.speaker ? idMap.get(c.speaker) ?? null : null }));
    if (run({ op: "addScene", scene: copy, index: i + 1 })) selectScene(i + 1);
  }
  function deleteScene(i: number) {
    run({ op: "removeScene", id: project!.scenes[i].id });
  }

  // Drag to reorder scenes
  let dragScene = $state<number | null>(null);
  function dropScene(i: number) {
    if (dragScene === null || dragScene === i) return;
    run({ op: "moveScene", id: project!.scenes[dragScene].id, to: i });
    app.sceneIndex = i;
    dragScene = null;
  }

  // ------------------------------------------------------------ tracks
  let ruler: HTMLDivElement | undefined = $state();
  const dur = $derived(scene?.duration ?? 1);
  const pct = (f: number) => `${(f / dur) * 100}%`;

  function frameFromX(clientX: number) {
    const r = ruler!.getBoundingClientRect();
    return Math.round(Math.min(1, Math.max(0, (clientX - r.left) / r.width)) * (dur - 1));
  }
  function scrub(ev: PointerEvent) {
    stop();
    app.frame = frameFromX(ev.clientX);
    const move = (e: PointerEvent) => (app.frame = frameFromX(e.clientX));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  /** Drag a bar's body (move in time) or its edges (trim). */
  function dragBar(ev: PointerEvent, item: Item, mode: "move" | "start" | "end") {
    ev.stopPropagation();
    app.selected = item.id;
    const f0 = frameFromX(ev.clientX);
    const orig = snap(item);
    const key = `bar-${Date.now()}`;
    const move = (e: PointerEvent) => {
      const d = frameFromX(e.clientX) - f0;
      const it = structuredClone(orig);
      const end = orig.end ?? dur;
      if (mode === "move") {
        const len = end - orig.start;
        it.start = Math.max(0, Math.min(dur - 1, orig.start + d));
        if (orig.end !== null) it.end = it.start + len;
        if (it.motion && orig.motion) {
          it.motion.start = Math.max(0, orig.motion.start + (it.start - orig.start));
          it.motion.end = it.motion.start + (orig.motion.end - orig.motion.start);
        }
      } else if (mode === "start") {
        it.start = Math.max(0, Math.min(end - 1, orig.start + d));
      } else {
        const e2 = Math.max(orig.start + 1, Math.min(dur, end + d));
        it.end = e2 >= dur ? null : e2;
      }
      updateItem(it, key);
    };
    const up = () => {
      endGesture();
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function dragClip(ev: PointerEvent, clip: AudioClip) {
    ev.stopPropagation();
    const f0 = frameFromX(ev.clientX);
    const orig = snap(clip);
    const key = `clip-${Date.now()}`;
    const move = (e: PointerEvent) => {
      const c = structuredClone(orig);
      c.start = Math.max(0, Math.min(dur - 1, orig.start + frameFromX(e.clientX) - f0));
      run({ op: "updateAudio", scene: scene!.id, clip: c }, key);
    };
    const up = () => {
      endGesture();
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  const ticks = $derived.by(() => {
    const s = secs(dur);
    const step = s > 40 ? 5 : s > 15 ? 2 : 1;
    const out: number[] = [];
    for (let t = 0; t <= s; t += step) out.push(t);
    return out;
  });
  const icon = (it: Item) => (it.kind.type === "character" ? "🧑" : it.kind.type === "prop" ? "🪴" : "💬");
  const clipLen = (c: AudioClip) => {
    const m = project?.media[c.media];
    if (!m) return 1;
    return Math.min(c.length ?? m.durationFrames - c.trimStart, m.durationFrames - c.trimStart);
  };
  const transIcon: Record<string, string> = { cut: "│", fade: "◐", fadeBlack: "●", slide: "⇢", zoom: "⊕" };
</script>

<section class="timeline panel">
  <div class="scenes">
    {#each project?.scenes ?? [] as s, i (s.id)}
      {#if i > 0}
        <button
          class="trans"
          title="Transition: {s.transition.kind} (click to change)"
          onclick={() => {
            const order = ["cut", "fade", "fadeBlack", "slide", "zoom"] as const;
            const next = order[(order.indexOf(s.transition.kind) + 1) % order.length];
            run({ op: "updateScene", scene: { ...snap(s), transition: { ...s.transition, kind: next } } });
          }}>{transIcon[s.transition.kind]}</button>
      {/if}
      <div
        class="scene-card"
        class:active={i === app.sceneIndex}
        class:dragging={dragScene === i}
        draggable="true"
        ondragstart={() => (dragScene = i)}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => dropScene(i)}
        onclick={() => selectScene(i)}
        onkeydown={(e) => e.key === "Enter" && selectScene(i)}
        role="button"
        tabindex="0"
      >
        {#if thumbs[i]}<img alt="" src={thumbs[i]} />{:else}<div class="ph"></div>{/if}
        <div class="meta">
          <span class="nm">{i + 1}. {s.name}</span>
          <span class="len">{secs(s.duration).toFixed(1)}s</span>
        </div>
        <div class="tools">
          <button title="Duplicate scene" onclick={(e) => { e.stopPropagation(); duplicateScene(i); }}>⧉</button>
          {#if (project?.scenes.length ?? 0) > 1}
            <button title="Delete scene" onclick={(e) => { e.stopPropagation(); deleteScene(i); }}>✕</button>
          {/if}
        </div>
      </div>
    {/each}
    <button class="add-scene" onclick={addScene} title="Add a scene after this one">＋<span>Scene</span></button>
  </div>

  {#if scene}
    <div class="tracks">
      <div class="labels">
        <div class="lab head">
          <button class="playbtn" onclick={() => (app.playing ? stop() : play("scene"))} title="Play scene (Space)">{app.playing && app.playMode === "scene" ? "❚❚" : "▶"}</button>
          <span class="time">{fmtTime(app.frame)} / {fmtTime(dur)}</span>
        </div>
        {#each [...scene.items].reverse() as it (it.id)}
          <button class="lab" class:sel={app.selected === it.id} onclick={() => (app.selected = it.id)}>
            <span>{icon(it)}</span><span class="nm">{it.name}</span>
          </button>
        {/each}
        {#each scene.audio as c (c.id)}
          {@const m = project?.media[c.media]}
          {@const who = scene.items.find((i) => i.id === c.speaker)?.name}
          <div class="lab audio" title={m?.source?.kind === "tts" ? m.source.text : m?.name}><span>{who ? "🗨" : "♪"}</span><span class="nm">{who ? `${who}: ` : ""}{m?.source?.kind === "tts" ? m.source.text : m?.name}</span></div>
        {/each}
      </div>
      <div class="lanes">
        <div class="ruler" bind:this={ruler} onpointerdown={scrub} role="slider" aria-valuenow={app.frame} aria-label="Playhead" tabindex="0">
          {#each ticks as t}
            <span class="tick" style:left={pct(frames(t))}>{t}s</span>
          {/each}
        </div>
        {#each [...scene.items].reverse() as it (it.id)}
          <div class="lane" onpointerdown={scrub} role="presentation">
            <div
              class="bar {it.kind.type}"
              class:sel={app.selected === it.id}
              style:left={pct(it.start)}
              style:width={pct((it.end ?? dur) - it.start)}
              onpointerdown={(e) => dragBar(e, it, "move")}
              role="presentation"
            >
              <span class="edge l" onpointerdown={(e) => dragBar(e, it, "start")} role="presentation"></span>
              {#if it.motion}
                <span class="mot" style:left="{((it.motion.start - it.start) / Math.max(1, (it.end ?? dur) - it.start)) * 100}%" style:width="{((it.motion.end - it.motion.start) / Math.max(1, (it.end ?? dur) - it.start)) * 100}%"></span>
              {/if}
              <span class="edge r" onpointerdown={(e) => dragBar(e, it, "end")} role="presentation"></span>
            </div>
          </div>
        {/each}
        {#each scene.audio as c (c.id)}
          <div class="lane" onpointerdown={scrub} role="presentation">
            <div class="bar audio" style:left={pct(c.start)} style:width={pct(Math.min(clipLen(c), dur - c.start))} onpointerdown={(e) => dragClip(e, c)} role="presentation">
              <span class="wave" style:--n={project?.media[c.media]?.durationFrames ?? 1}></span>
            </div>
          </div>
        {/each}
        <div class="playhead" style:left={pct(app.frame)}></div>
      </div>
    </div>
  {/if}
</section>

<style>
  .timeline {
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    height: 290px;
    min-height: 200px;
  }
  .scenes {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 12px;
    overflow-x: auto;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .scene-card {
    position: relative;
    flex-shrink: 0;
    width: 132px;
    border-radius: 9px;
    border: 2px solid transparent;
    background: var(--bg-2);
    cursor: pointer;
    overflow: hidden;
  }
  .scene-card.active {
    border-color: var(--accent);
  }
  .scene-card.dragging {
    opacity: 0.5;
  }
  .scene-card img,
  .ph {
    display: block;
    width: 128px;
    height: 72px;
    background: var(--bg-3);
  }
  .meta {
    display: flex;
    justify-content: space-between;
    padding: 4px 6px;
    font-size: 11px;
  }
  .meta .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 84px;
  }
  .len {
    color: var(--muted);
  }
  .tools {
    position: absolute;
    top: 4px;
    right: 4px;
    display: none;
    gap: 3px;
  }
  .scene-card:hover .tools {
    display: flex;
  }
  .tools button {
    padding: 1px 6px;
    font-size: 11px;
    background: rgba(20, 22, 34, 0.85);
  }
  .trans {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    padding: 0;
    border-radius: 50%;
    font-size: 12px;
    color: var(--accent-2);
  }
  .add-scene {
    flex-shrink: 0;
    width: 72px;
    height: 96px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    border-style: dashed;
  }
  .add-scene span {
    font-size: 11px;
    color: var(--muted);
  }
  .tracks {
    flex: 1;
    display: flex;
    overflow: auto;
    min-height: 0;
  }
  .labels {
    width: 180px;
    flex-shrink: 0;
    border-right: 1px solid var(--line);
  }
  .lab {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 10px;
    width: 100%;
    font-size: 12px;
    background: none;
    border: none;
    border-radius: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    text-align: left;
    color: var(--text);
  }
  .lab.sel {
    background: rgba(91, 108, 255, 0.16);
  }
  .lab .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lab.head {
    height: 30px;
    position: sticky;
    top: 0;
    background: var(--bg-1);
    z-index: 2;
  }
  .lab.audio {
    color: #ffcf5a;
  }
  .playbtn {
    width: 26px;
    height: 22px;
    padding: 0;
    font-size: 11px;
  }
  .time {
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .lanes {
    position: relative;
    flex: 1;
    min-width: 400px;
    margin-right: 12px;
  }
  .ruler {
    position: sticky;
    top: 0;
    height: 30px;
    background: var(--bg-1);
    border-bottom: 1px solid var(--line);
    cursor: ew-resize;
    z-index: 2;
  }
  .tick {
    position: absolute;
    top: 8px;
    font-size: 10.5px;
    color: var(--muted);
    transform: translateX(-50%);
    pointer-events: none;
  }
  .tick::after {
    content: "";
    position: absolute;
    left: 50%;
    top: 16px;
    height: 6px;
    border-left: 1px solid var(--line);
  }
  .lane {
    position: relative;
    height: 28px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }
  .bar {
    position: absolute;
    top: 5px;
    bottom: 5px;
    border-radius: 6px;
    background: #4a5bd6;
    cursor: grab;
    min-width: 6px;
    opacity: 0.85;
  }
  .bar.prop { background: #2f8f5d; }
  .bar.text { background: #b88a1f; }
  .bar.audio { background: #6b5a1d; border: 1px solid #ffcf5a55; }
  .bar.sel {
    opacity: 1;
    box-shadow: 0 0 0 2px white;
  }
  .edge {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 7px;
    cursor: ew-resize;
  }
  .edge.l { left: 0; }
  .edge.r { right: 0; }
  .mot {
    position: absolute;
    top: 6px;
    height: 6px;
    border-radius: 3px;
    background: #ffcf5a;
    pointer-events: none;
  }
  .wave {
    position: absolute;
    inset: 3px 4px;
    background: repeating-linear-gradient(90deg, #ffcf5a88 0 2px, transparent 2px 5px);
    mask: linear-gradient(transparent 20%, black 40%, black 60%, transparent 80%);
  }
  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: #ff5d73;
    pointer-events: none;
    z-index: 3;
  }
  .playhead::before {
    content: "";
    position: absolute;
    top: 0;
    left: -5px;
    border: 6px solid transparent;
    border-top-color: #ff5d73;
  }
</style>
