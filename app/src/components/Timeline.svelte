<script lang="ts">
  import { app, currentScene, run, updateItem, endGesture, fmtTime, secs, frames, keyFrames } from "../lib/store.svelte";
  import type { Item as ItemT } from "../lib/types";
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

  /**
   * Drag a keyframe diamond to retime that moment (every property keyed at it
   * moves together), e.g. stand still until 3 s and only then start walking.
   * A click without dragging jumps the playhead there.
   */
  function dragKey(ev: PointerEvent, it: ItemT, f0: number) {
    ev.stopPropagation();
    app.selected = it.id;
    const all = keyFrames(it);
    const i = all.indexOf(f0);
    const lo = i > 0 ? all[i - 1] + 1 : 0;
    const hi = i < all.length - 1 ? all[i + 1] - 1 : dur - 1;
    const x0 = ev.clientX;
    let cur = f0;
    let moved = false;
    const key = `key-${Date.now()}`;
    const move = (e: PointerEvent) => {
      if (!moved && Math.abs(e.clientX - x0) < 4) return;
      moved = true;
      const f = Math.min(hi, Math.max(lo, frameFromX(e.clientX)));
      if (f === cur) return;
      if (run({ op: "moveKeys", scene: scene!.id, item: it.id, from: cur, to: f }, key)) {
        cur = f;
        app.frame = f;
      }
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (moved) endGesture();
      else app.frame = f0;
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
        // Keyframes travel with the item.
        const shift = it.start - orig.start;
        for (const [p, t] of Object.entries(orig.tracks ?? {})) {
          it.tracks![p] = { keys: t.keys.map((k) => ({ ...k, frame: Math.max(0, k.frame + shift) })) };
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

  // ------------------------------------------------------------ panel size
  // The timeline's height is dragged from its top edge; it can also collapse
  // to just the scene strip. Both are remembered.
  const store = {
    get(k: string) {
      try {
        return localStorage.getItem(k);
      } catch {
        return null;
      }
    },
    set(k: string, v: string) {
      try {
        localStorage.setItem(k, v);
      } catch {
        /* private mode */
      }
    },
  };
  const MIN_H = 170;
  const maxH = () => Math.max(MIN_H, window.innerHeight - 240);
  let height = $state(Math.min(Number(store.get("frameling.timelineHeight")) || 290, maxH()));
  let collapsed = $state(store.get("frameling.timelineCollapsed") === "1");
  function setCollapsed(v: boolean) {
    collapsed = v;
    store.set("frameling.timelineCollapsed", v ? "1" : "0");
  }
  function resizeStart(ev: PointerEvent) {
    ev.preventDefault();
    const y0 = ev.clientY;
    const h0 = collapsed ? MIN_H : height;
    let moved = false;
    const move = (e: PointerEvent) => {
      const dy = y0 - e.clientY;
      if (Math.abs(dy) > 3) moved = true;
      if (!moved) return;
      // Dragging a collapsed timeline up opens it; dragging it far down collapses it.
      const h = h0 + dy;
      if (h < MIN_H - 60) {
        setCollapsed(true);
      } else {
        if (collapsed) setCollapsed(false);
        height = Math.round(Math.min(maxH(), Math.max(MIN_H, h)));
      }
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      store.set("frameling.timelineHeight", String(height));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
  $effect(() => {
    const fit = () => (height = Math.min(height, maxH()));
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  });

  // ------------------------------------------------------------ zoom
  // 1 = the whole scene fits; higher zooms in and the lanes scroll sideways.
  let zoom = $state(1);
  let tracksEl: HTMLDivElement | undefined = $state();
  let viewW = $state(800); // width available to the lanes at zoom 1
  const LABELS_W = 200;
  $effect(() => {
    if (!tracksEl) return;
    const ro = new ResizeObserver(() => (viewW = Math.max(400, tracksEl!.clientWidth - LABELS_W - 12)));
    ro.observe(tracksEl);
    return () => ro.disconnect();
  });
  /** Up to about 300 px per second, at least 2x. */
  const maxZoom = $derived(Math.max(2, Math.min(80, (300 * secs(dur)) / viewW)));
  const laneW = $derived(viewW * zoom);

  function setZoom(z: number, anchorClientX?: number) {
    const el = tracksEl;
    const nz = Math.min(maxZoom, Math.max(1, z));
    if (!el || nz === zoom) return;
    // Keep the time under the anchor (pointer, or the playhead) where it is.
    const r = el.getBoundingClientRect();
    const ax = anchorClientX !== undefined ? anchorClientX - r.left - LABELS_W : (app.frame / dur) * laneW - el.scrollLeft;
    const t = (el.scrollLeft + ax) / laneW;
    zoom = nz;
    requestAnimationFrame(() => (el.scrollLeft = Math.max(0, t * viewW * nz - ax)));
  }
  function onWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return; // plain wheel scrolls; Ctrl/Cmd (or pinch) zooms
    e.preventDefault();
    setZoom(zoom * Math.exp(-e.deltaY * 0.004), e.clientX);
  }
  // Keep the playhead on screen while playing or scrubbing past the edge.
  $effect(() => {
    const el = tracksEl;
    const f = app.frame;
    if (!el || zoom === 1) return;
    const x = (f / dur) * laneW;
    const left = el.scrollLeft;
    const visible = el.clientWidth - LABELS_W - 12;
    if (x < left || x > left + visible) el.scrollLeft = Math.max(0, x - (app.playing ? 40 : visible / 2));
  });

  const ticks = $derived.by(() => {
    const s = secs(dur);
    // At least ~70 px between labels at the current zoom.
    const pxPerSec = laneW / Math.max(s, 0.001);
    const step = [0.5, 1, 2, 5, 10, 15, 30, 60, 120].find((st) => st * pxPerSec >= 70) ?? 300;
    const out: number[] = [];
    for (let t = 0; t <= s + 1e-6; t += step) out.push(Math.round(t * 10) / 10);
    return out;
  });
  const tickLabel = (t: number) => (t >= 60 ? `${Math.floor(t / 60)}:${String(Math.round(t % 60)).padStart(2, "0")}` : `${t}s`);
  const icon = (it: Item) => (it.kind.type === "character" ? "🧑" : it.kind.type === "prop" ? "🪴" : "💬");
  const clipLen = (c: AudioClip) => {
    const m = project?.media[c.media];
    if (!m) return 1;
    return Math.min(c.length ?? m.durationFrames - c.trimStart, m.durationFrames - c.trimStart);
  };
  const transIcon: Record<string, string> = { cut: "│", fade: "◐", fadeBlack: "●", slide: "⇢", zoom: "⊕" };
</script>

<section class="timeline panel" class:collapsed style:height={collapsed ? "auto" : `${height}px`}>
  <div
    class="grip"
    onpointerdown={resizeStart}
    ondblclick={() => setCollapsed(!collapsed)}
    role="separator"
    aria-orientation="horizontal"
    aria-label="Resize timeline"
    title="Drag to resize the timeline, double-click to collapse or expand"
  ></div>
  <div class="strip">
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
    <button class="collapse" onclick={() => setCollapsed(!collapsed)} title={collapsed ? "Show the timeline tracks" : "Hide the timeline tracks"} aria-expanded={!collapsed}>
      {collapsed ? "▴ Show tracks" : "▾ Hide tracks"}
    </button>
  </div>

  {#if scene && !collapsed}
    <div class="tracks" bind:this={tracksEl} onwheel={onWheel}>
      <div class="labels">
        <div class="lab head">
          <button class="playbtn" onclick={() => (app.playing ? stop() : play("scene"))} title="Play scene (Space)">{app.playing && app.playMode === "scene" ? "❚❚" : "▶"}</button>
          <span class="time">{fmtTime(app.frame)} / {fmtTime(dur)}</span>
          <span class="zoom">
            <button onclick={() => setZoom(zoom / 1.6)} disabled={zoom <= 1} title="Zoom out (Ctrl/⌘ + scroll)" aria-label="Zoom out">−</button>
            <button onclick={() => setZoom(zoom * 1.6)} disabled={zoom >= maxZoom} title="Zoom in (Ctrl/⌘ + scroll)" aria-label="Zoom in">+</button>
            <button onclick={() => setZoom(1)} disabled={zoom <= 1} title="Fit the whole scene" aria-label="Fit scene">⤢</button>
          </span>
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
      <div class="lanes" style:width="{laneW}px">
        <div class="ruler" bind:this={ruler} onpointerdown={scrub} role="slider" aria-valuenow={app.frame} aria-label="Playhead" tabindex="0">
          {#each ticks as t}
            <span class="tick" style:left={pct(frames(t))}>{tickLabel(t)}</span>
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
            {#if it.kind.type === "character"}
              {#each it.kind.poses ?? [] as s, si (si)}
                <span
                  class="pstrip"
                  style:left={pct(s.start)}
                  style:width={pct(Math.max(1, s.end - s.start))}
                  style:--bi="{(s.blendIn / Math.max(1, s.end - s.start)) * 100}%"
                  style:--bo="{(s.blendOut / Math.max(1, s.end - s.start)) * 100}%"
                  title="Pose: {s.pose} {fmtTime(s.start)}–{fmtTime(s.end)}{s.mask !== 'full' ? ` (${s.mask})` : ''}"
                >{s.pose}</span>
              {/each}
            {/if}
            {#each keyFrames(it) as f (f)}
              <button
                class="kf"
                class:sel={app.selected === it.id}
                class:here={f === app.frame}
                style:left={pct(f)}
                title="Keyframe at {fmtTime(f)}: click to jump here, drag to change when it happens"
                aria-label="Keyframe at {fmtTime(f)}"
                onpointerdown={(e) => dragKey(e, it, f)}
              ></button>
            {/each}
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
  .pstrip {
    position: absolute;
    top: 15px;
    height: 11px;
    border-radius: 3px;
    background: linear-gradient(90deg, transparent, #c58cff var(--bi), #c58cff calc(100% - var(--bo)), transparent);
    color: #1c1030;
    font-size: 8.5px;
    line-height: 11px;
    padding: 0 4px;
    overflow: hidden;
    white-space: nowrap;
    pointer-events: none;
    z-index: 1;
  }
  .kf {
    position: absolute;
    top: 50%;
    width: 9px;
    height: 9px;
    padding: 0;
    margin: -5px 0 0 -5px;
    transform: rotate(45deg);
    background: #ffcf5a;
    border: 1px solid #1c2032;
    border-radius: 1px;
    opacity: 0.7;
    z-index: 2;
    cursor: ew-resize;
  }
  .kf.sel {
    opacity: 1;
  }
  .kf.here {
    background: #fff;
    box-shadow: 0 0 0 2px #ffcf5a;
  }
  .timeline {
    position: relative;
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }
  .grip {
    position: absolute;
    top: -4px;
    left: 0;
    right: 0;
    height: 8px;
    cursor: ns-resize;
    z-index: 5;
  }
  .grip::after {
    content: "";
    position: absolute;
    left: 50%;
    top: 2px;
    width: 40px;
    height: 4px;
    margin-left: -20px;
    border-radius: 2px;
    background: var(--line);
    transition: background 0.15s;
  }
  .grip:hover::after {
    background: var(--accent);
  }
  .strip {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .strip .scenes {
    flex: 1;
    min-width: 0;
    border-bottom: none;
  }
  .collapse {
    flex-shrink: 0;
    margin: 0 12px 0 4px;
    font-size: 11px;
    padding: 5px 10px;
    white-space: nowrap;
    color: var(--muted);
  }
  .collapsed .strip {
    border-bottom: none;
  }
  .zoom {
    display: flex;
    gap: 2px;
    margin-left: auto;
  }
  .zoom button {
    width: 20px;
    height: 20px;
    padding: 0;
    font-size: 12px;
    line-height: 1;
  }
  .zoom button:disabled {
    opacity: 0.35;
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
    width: 200px;
    flex-shrink: 0;
    border-right: 1px solid var(--line);
    /* Stay put while the lanes scroll sideways. */
    position: sticky;
    left: 0;
    z-index: 4;
    background: var(--bg-1);
    height: max-content;
    min-height: 100%;
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
    gap: 6px;
    padding-right: 6px;
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
    white-space: nowrap;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .lanes {
    position: relative;
    flex: none;
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
