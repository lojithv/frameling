<script lang="ts">
  import { app, eng, currentScene, selectedItem, selectedPosed, posedItem, updateItem, editItem, endGesture, run, toast, positionKeyFrames, setPositionKey, fmtTime } from "../lib/store.svelte";
  import { newCharacter, newProp, newText, CHARACTER_PRESETS } from "../lib/defaults";
  import type { Item, ItemHit, Scene } from "../lib/types";
  import { toImageData } from "../lib/thumbs";

  let wrap: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let box = $state({ w: 640, h: 360 });
  let pending = false;

  const project = $derived(app.project);
  const scene = $derived(currentScene());
  const raw = $derived(selectedItem());
  /** The selection as it looks at the playhead (keyed values applied). */
  const sel = $derived(selectedPosed());
  const k = $derived(project ? box.w / project.width : 1); // css px per project px

  // Fit a 16:9 (or project aspect) stage into the available space.
  $effect(() => {
    const ro = new ResizeObserver(() => {
      if (!project) return;
      const r = wrap.getBoundingClientRect();
      const aspect = project.width / project.height;
      let w = r.width - 32;
      let h = w / aspect;
      if (h > r.height - 32) {
        h = r.height - 32;
        w = h * aspect;
      }
      box = { w: Math.max(160, Math.floor(w)), h: Math.max(90, Math.floor(h)) };
    });
    ro.observe(wrap);
    return () => ro.disconnect();
  });

  // Redraw whenever anything visible changes.
  $effect(() => {
    void app.rev;
    void app.frame;
    void app.sceneIndex;
    void app.globalFrame;
    void box.w;
    if (!pending) {
      pending = true;
      requestAnimationFrame(draw);
    }
  });

  function draw() {
    pending = false;
    if (!project || !canvas) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const cap = app.playing ? 960 : 1600;
    const pw = Math.min(Math.round(box.w * dpr), cap, project.width * 2);
    const ph = Math.round((pw * project.height) / project.width);
    if (canvas.width !== pw || canvas.height !== ph) {
      canvas.width = pw;
      canvas.height = ph;
    }
    const e = eng();
    const px =
      app.playing && app.playMode === "all"
        ? e.render_frame(app.globalFrame, pw, ph)
        : e.render_scene(app.sceneIndex, app.frame, pw, ph);
    const img = toImageData(px, pw, ph);
    canvas.getContext("2d")!.putImageData(img, 0, 0);
    if (!(app.playing && app.playMode === "all")) app.hits = JSON.parse(e.last_hits());
  }

  // ------------------------------------------------------------ camera math
  /** Camera at a local frame, from the engine (includes camera keyframes). */
  function camAt(s: Scene, f: number) {
    const i = app.project?.scenes.findIndex((x) => x.id === s.id) ?? -1;
    if (app.ready && i >= 0) {
      const [x, y, zoom] = eng().camera_at(i, f);
      return { x, y, zoom };
    }
    return s.camera.from;
  }
  /** Scene (world) coords -> stage css px. */
  function toScreen(x: number, y: number) {
    const c = camAt(scene!, app.frame);
    return { x: ((x - c.x) * c.zoom + project!.width / 2) * k, y: ((y - c.y) * c.zoom + project!.height / 2) * k };
  }
  /** Stage css px -> scene (world) coords. */
  function toWorld(sx: number, sy: number) {
    const c = camAt(scene!, app.frame);
    return { x: (sx / k - project!.width / 2) / c.zoom + c.x, y: (sy / k - project!.height / 2) / c.zoom + c.y };
  }

  // ------------------------------------------------------------ interaction
  type Drag =
    | { kind: "move"; item: Item; sx: number; sy: number; target: "base" | "motion" | "legacy"; key: string }
    | { kind: "scale"; item: Item; ax: number; ay: number; d0: number; key: string }
    | { kind: "rotate"; item: Item; ax: number; ay: number; a0: number; key: string };

  /** Items still using the old one-segment movement (no position keys) keep editing it directly. */
  const legacyMotion = (it: Item) => !!it.motion && !it.tracks?.["transform.x"] && !it.tracks?.["transform.y"];
  let drag: Drag | null = null;

  function local(ev: PointerEvent | DragEvent) {
    const r = canvas.getBoundingClientRect();
    return { x: ev.clientX - r.left, y: ev.clientY - r.top };
  }

  function hitAt(x: number, y: number): ItemHit | null {
    const px = x / k;
    const py = y / k;
    const inside = (h: ItemHit) =>
      !h.bounds || h.bounds.w < 0 ? false : px >= h.bounds.x && px <= h.bounds.x + h.bounds.w && py >= h.bounds.y && py <= h.bounds.y + h.bounds.h;
    const hits = [...app.hits].reverse();
    return hits.find((h) => h.visible && inside(h)) ?? hits.find(inside) ?? null;
  }

  function onDown(ev: PointerEvent) {
    if (app.playing || !scene) return;
    const p = local(ev);
    const h = hitAt(p.x, p.y);
    app.selected = h?.id ?? null;
    const it = scene.items.find((i) => i.id === h?.id);
    if (!it || it.locked) return;
    const legacy = legacyMotion(it);
    const target = legacy ? (app.frame >= it.motion!.end ? "motion" : "legacy") : "base";
    const start = legacy ? (structuredClone($state.snapshot(it)) as Item) : structuredClone(posedItem(it, app.frame));
    drag = { kind: "move", item: start, sx: p.x, sy: p.y, target, key: `move-${Date.now()}` };
    canvas.setPointerCapture(ev.pointerId);
  }

  function startHandle(ev: PointerEvent, kind: "scale" | "rotate" | "motion") {
    ev.stopPropagation();
    if (!sel || sel.locked) return;
    const item = structuredClone($state.snapshot(sel)) as Item;
    const a = toScreen(item.x, item.y);
    const p = local(ev);
    if (kind === "motion") {
      drag = { kind: "move", item: structuredClone($state.snapshot(raw!)) as Item, sx: p.x, sy: p.y, target: "motion", key: `motion-${Date.now()}` };
    } else if (kind === "scale") {
      drag = { kind, item, ax: a.x, ay: a.y, d0: Math.hypot(p.x - a.x, p.y - a.y) || 1, key: `scale-${Date.now()}` };
    } else {
      drag = { kind, item, ax: a.x, ay: a.y, a0: Math.atan2(p.y - a.y, p.x - a.x), key: `rot-${Date.now()}` };
    }
    (ev.target as HTMLElement).setPointerCapture(ev.pointerId);
  }

  function onMove(ev: PointerEvent) {
    if (!drag) return;
    const p = local(ev);
    const it = structuredClone(drag.item);
    if (drag.kind === "move") {
      const zoom = camAt(scene!, app.frame).zoom;
      const dx = (p.x - drag.sx) / k / zoom;
      const dy = (p.y - drag.sy) / k / zoom;
      if (drag.target === "motion" && it.motion) {
        it.motion.x = Math.round(it.motion.x + dx);
        it.motion.y = Math.round(it.motion.y + dy);
      } else {
        it.x = Math.round(it.x + dx);
        it.y = Math.round(it.y + dy);
      }
    } else if (drag.kind === "scale") {
      const d = Math.hypot(p.x - drag.ax, p.y - drag.ay);
      it.scale = Math.max(0.05, Math.round(((drag.item.scale * d) / drag.d0) * 100) / 100);
    } else {
      const a = Math.atan2(p.y - drag.ay, p.x - drag.ax);
      let deg = drag.item.rotation + ((a - drag.a0) * 180) / Math.PI;
      if (ev.shiftKey) deg = Math.round(deg / 15) * 15;
      it.rotation = Math.round(deg);
    }
    // Old-style movement edits the document item as is; everything else is an
    // edit at the playhead, which auto-keys.
    if (drag.kind === "move" && drag.target !== "base") updateItem(it, drag.key);
    else editItem(it, drag.key);
  }

  function onUp() {
    if (drag) endGesture();
    drag = null;
  }

  // ------------------------------------------------------------ drop from library
  function onDrop(ev: DragEvent) {
    ev.preventDefault();
    const raw = ev.dataTransfer?.getData("application/x-frameling");
    if (!raw || !scene) return;
    const p = local(ev);
    const w = toWorld(p.x, p.y);
    const d = JSON.parse(raw);
    let item: Item | null = null;
    if (d.kind === "prop") item = newProp(d.id, d.name, Math.round(w.x), Math.round(w.y));
    if (d.kind === "character") {
      const preset = CHARACTER_PRESETS[d.index];
      item = newCharacter(preset.look, preset.name, Math.round(w.x), Math.round(w.y));
    }
    if (d.kind === "text") item = newText(d.style, d.sample, Math.round(w.x), Math.round(w.y));
    if (d.kind === "background") {
      run({ op: "updateScene", scene: { ...$state.snapshot(scene), background: { preset: d.id, colors: [] } } as Scene });
      return;
    }
    if (item) {
      item.start = Math.min(app.frame, scene.duration - 1) > 0 && ev.altKey ? app.frame : 0;
      if (run({ op: "addItem", scene: scene.id, item })) app.selected = item.id;
    }
  }

  const selHit = $derived(app.hits.find((h) => h.id === app.selected && h.bounds && h.bounds.w >= 0));
  const motionMarker = $derived.by(() => {
    if (!raw || !legacyMotion(raw) || !scene || !project) return null;
    void app.frame;
    const a = toScreen(raw.x, raw.y);
    const b = toScreen(raw.motion!.x, raw.motion!.y);
    return { a, b };
  });

  // ------------------------------------------------------------ movement path
  // The selected item's keyed path, with a draggable point at each position
  // key: the first one is where (and, on the timeline, when) the move starts.
  const path = $derived.by(() => {
    void app.rev;
    void app.frame;
    if (!raw || !scene || !project || app.playing) return null;
    const keys = positionKeyFrames(raw);
    if (keys.length < 2) return null;
    const first = keys[0];
    const last = keys[keys.length - 1];
    const step = Math.max(1, Math.round((last - first) / 80));
    const pts: string[] = [];
    for (let f = first; f <= last; f += step) {
      const p = posedItem(raw, f);
      const s = toScreen(p.x, p.y);
      pts.push(`${s.x.toFixed(1)},${s.y.toFixed(1)}`);
    }
    const end = posedItem(raw, last);
    const se = toScreen(end.x, end.y);
    pts.push(`${se.x.toFixed(1)},${se.y.toFixed(1)}`);
    const dots = keys.map((f, i) => {
      const p = posedItem(raw, f);
      const s = toScreen(p.x, p.y);
      return { f, x: s.x, y: s.y, wx: p.x, wy: p.y, label: i === 0 ? `Start ${fmtTime(f)}` : i === keys.length - 1 ? `End ${fmtTime(f)}` : fmtTime(f) };
    });
    return { points: pts.join(" "), dots };
  });

  function dragPoint(ev: PointerEvent, d: { f: number; wx: number; wy: number }) {
    ev.stopPropagation();
    ev.preventDefault();
    const el = ev.currentTarget as HTMLElement;
    el.setPointerCapture(ev.pointerId);
    const p0 = local(ev);
    const key = `pt-${Date.now()}`;
    let moved = false;
    const move = (e: PointerEvent) => {
      const p = local(e);
      if (!moved && Math.hypot(p.x - p0.x, p.y - p0.y) < 3) return;
      moved = true;
      const zoom = camAt(scene!, app.frame).zoom;
      const it = selectedItem();
      if (it) setPositionKey(it, d.f, d.wx + (p.x - p0.x) / k / zoom, d.wy + (p.y - p0.y) / k / zoom, key);
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      if (moved) endGesture();
      else app.frame = d.f;
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  function onDblClick() {
    if (sel?.kind.type === "text") document.getElementById("text-input")?.focus();
  }

  $effect(() => {
    if (app.project && app.project.scenes.length === 0) toast("Add a scene to begin", "info");
  });
</script>

<div class="stage-wrap" bind:this={wrap}>
  <div
    class="stage"
    style:width="{box.w}px"
    style:height="{box.h}px"
    ondragover={(e) => e.preventDefault()}
    ondrop={onDrop}
    role="application"
    aria-label="Stage"
  >
    <canvas
      bind:this={canvas}
      style:width="{box.w}px"
      style:height="{box.h}px"
      onpointerdown={onDown}
      onpointermove={onMove}
      onpointerup={onUp}
      onpointercancel={onUp}
      ondblclick={onDblClick}
    ></canvas>

    {#if !app.playing}
      {#if path}
        <svg class="motion" width={box.w} height={box.h}>
          <polyline points={path.points} />
        </svg>
        {#each path.dots as d, i (d.f)}
          <button
            class="path-pt"
            class:start={i === 0}
            class:here={d.f === app.frame}
            style:left="{d.x}px"
            style:top="{d.y}px"
            title="{d.label}: drag to move this point, click to go to {fmtTime(d.f)}. Change when it happens by dragging its diamond on the timeline."
            aria-label="Movement point at {fmtTime(d.f)}"
            onpointerdown={(e) => dragPoint(e, d)}
          ><span>{d.label}</span></button>
        {/each}
      {/if}
      {#if motionMarker}
        <svg class="motion" width={box.w} height={box.h}>
          <line x1={motionMarker.a.x} y1={motionMarker.a.y} x2={motionMarker.b.x} y2={motionMarker.b.y} />
          <circle cx={motionMarker.a.x} cy={motionMarker.a.y} r="5" class="from" />
        </svg>
        <button
          class="motion-target"
          title="Drag to set where it moves to"
          style:left="{motionMarker.b.x}px"
          style:top="{motionMarker.b.y}px"
          onpointerdown={(e) => startHandle(e, "motion")}
          onpointermove={onMove}
          onpointerup={onUp}
        >➜</button>
      {/if}
      {#if selHit && sel}
        {@const b = selHit.bounds}
        <div
          class="sel"
          class:locked={sel.locked}
          class:hidden={!selHit.visible}
          style:left="{b.x * k}px"
          style:top="{b.y * k}px"
          style:width="{b.w * k}px"
          style:height="{b.h * k}px"
        >
          {#if !sel.locked}
            <button
              class="handle scale"
              title="Drag to resize"
              aria-label="Resize"
              onpointerdown={(e) => startHandle(e, "scale")}
              onpointermove={onMove}
              onpointerup={onUp}
            ></button>
            <button
              class="handle rotate"
              title="Drag to rotate (Shift snaps)"
              aria-label="Rotate"
              onpointerdown={(e) => startHandle(e, "rotate")}
              onpointermove={onMove}
              onpointerup={onUp}
            >⟳</button>
          {/if}
          <span class="tag">{sel.name}{selHit.visible ? "" : " (hidden now)"}</span>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .stage-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    background:
      radial-gradient(circle at 50% 40%, rgba(91, 108, 255, 0.08), transparent 60%),
      var(--bg-0);
  }
  .stage {
    position: relative;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45), 0 0 0 1px rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    overflow: hidden;
    touch-action: none;
  }
  canvas {
    display: block;
    cursor: default;
  }
  .sel {
    position: absolute;
    border: 1.5px dashed var(--accent);
    border-radius: 4px;
    pointer-events: none;
  }
  .sel.locked {
    border-color: var(--muted);
  }
  .sel.hidden {
    opacity: 0.5;
  }
  .tag {
    position: absolute;
    left: -1px;
    top: -22px;
    font-size: 11px;
    background: var(--accent);
    color: white;
    padding: 2px 7px;
    border-radius: 4px 4px 4px 0;
    white-space: nowrap;
  }
  .handle {
    position: absolute;
    pointer-events: auto;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: white;
    border: 2px solid var(--accent);
    padding: 0;
  }
  .handle.scale {
    right: -8px;
    bottom: -8px;
    cursor: nwse-resize;
  }
  .handle.rotate {
    left: calc(50% - 11px);
    top: -36px;
    width: 22px;
    height: 22px;
    font-size: 13px;
    line-height: 16px;
    color: var(--accent);
    cursor: grab;
  }
  .motion {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .motion line {
    stroke: #ffcf5a;
    stroke-width: 2;
    stroke-dasharray: 6 5;
  }
  .motion polyline {
    fill: none;
    stroke: #ffcf5a;
    stroke-width: 2;
    stroke-dasharray: 6 5;
    opacity: 0.9;
  }
  .path-pt {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 14px;
    height: 14px;
    padding: 0;
    border-radius: 50%;
    background: #ffcf5a;
    border: 2px solid #fff;
    cursor: move;
    z-index: 3;
  }
  .path-pt.start {
    background: #3fae6a;
  }
  .path-pt.here {
    box-shadow: 0 0 0 3px rgba(255, 207, 90, 0.5);
  }
  .path-pt span {
    position: absolute;
    top: 14px;
    left: 50%;
    transform: translateX(-50%);
    white-space: nowrap;
    font-size: 10px;
    font-weight: 600;
    color: #fff;
    background: rgba(20, 23, 38, 0.8);
    padding: 1px 5px;
    border-radius: 4px;
    pointer-events: none;
  }
  .motion .from {
    fill: #ffcf5a;
  }
  .motion-target {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: #ffcf5a;
    color: #1d1f2e;
    border: 2px solid white;
    font-weight: 700;
    cursor: move;
    padding: 0;
  }
</style>
