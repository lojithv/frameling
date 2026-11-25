<script lang="ts">
  import { app, fmtTime, eng } from "../lib/store.svelte";
  import { sceneStarts } from "../lib/playback";
  import { untrack } from "svelte";
  import { exportVideo, exportPreviewEngine, cancelExport, isDesktop, type ExportSettings } from "../lib/platform";
  import { toImageData } from "../lib/thumbs";
  import type { Engine } from "../engine/frameling.js";

  let { open = $bindable(false) }: { open: boolean } = $props();
  const p = $derived(app.project!);

  /** One-click presets: where the video is going decides its shape and size. */
  const PRESETS = [
    { id: "shorts", icon: "📱", label: "Shorts · Reels · TikTok", hint: "Vertical 1080×1920, subtitles burned in", frame: [1080, 1920], out: [1080, 1920], captions: true },
    { id: "youtube", icon: "▶", label: "YouTube", hint: "Full HD 1920×1080", frame: [1920, 1080], out: [1920, 1080], captions: undefined },
    { id: "square", icon: "⬛", label: "Square post", hint: "1080×1080 for feeds, subtitles burned in", frame: [1080, 1080], out: [1080, 1080], captions: true },
    { id: "project", icon: "🎬", label: "As the project", hint: "", frame: null, out: null, captions: undefined },
  ] as const;
  let presetId = $state<(typeof PRESETS)[number]["id"]>("project");
  let quality = $state<"high" | "draft">("high");
  let format = $state<"mp4" | "webm">("mp4");
  let burnCaptions = $state(!!app.project?.captions?.enabled);
  let thumb = $state(true);
  let srt = $state(false);
  const preset = $derived(PRESETS.find((x) => x.id === presetId)!);
  $effect(() => {
    // Presets made for social feeds turn subtitles on; others keep the project's choice.
    burnCaptions = preset.captions ?? !!app.project?.captions?.enabled;
  });
  const frame = $derived(preset.frame ? { width: preset.frame[0], height: preset.frame[1] } : { width: p.width, height: p.height });
  const reshaped = $derived(frame.width / frame.height !== p.width / p.height);
  const size = $derived.by(() => {
    let [w, h] = preset.out ?? [Math.round((p.width * 1920) / Math.max(p.width, p.height)), Math.round((p.height * 1920) / Math.max(p.width, p.height))];
    if (quality === "draft") [w, h] = [w / 2, h / 2];
    return { w: Math.round(w / 2) * 2, h: Math.round(h / 2) * 2 };
  });
  /** The playhead as a global frame. */
  const playhead = $derived((sceneStarts()[app.sceneIndex] ?? 0) + app.frame);
  const totalFrames = $derived(app.project ? eng().total_frames() : 1);
  /** The thumbnail's frame: starts at the playhead each time the dialog opens. */
  let thumbFrame = $state(0);
  $effect(() => {
    if (open) thumbFrame = untrack(() => Math.min(playhead, totalFrames - 1));
  });

  // ------------------------------------------------------------ thumbnail preview
  // Rendered by an engine holding the export version of the project, so it
  // shows the export's frame shape, subtitles and packs.
  let canvas = $state<HTMLCanvasElement>();
  let preview = $state<Engine | null>(null);
  const previewSize = $derived.by(() => {
    const k = Math.min(200 / frame.width, 112 / frame.height);
    return { w: Math.max(2, Math.round(frame.width * k)), h: Math.max(2, Math.round(frame.height * k)) };
  });
  $effect(() => {
    if (!open || !thumb) return;
    const settings = { frame: reshaped ? frame : undefined, captions: burnCaptions };
    let made: Engine | null = null;
    let gone = false;
    exportPreviewEngine(settings).then((e) => {
      if (gone) return e.free();
      made = e;
      preview = e;
    }, () => {});
    return () => {
      gone = true;
      if (made) {
        if (preview === made) preview = null;
        made.free();
      }
    };
  });
  $effect(() => {
    const e = preview, c = canvas, f = thumbFrame, { w, h } = previewSize;
    if (!e || !c) return;
    const id = requestAnimationFrame(() => {
      const ratio = Math.min(2, window.devicePixelRatio || 1);
      const pw = Math.round(w * ratio), ph = Math.round(h * ratio);
      c.width = pw;
      c.height = ph;
      const px = e.render_frame(Math.max(0, Math.min(e.total_frames() - 1, f)), pw, ph);
      c.getContext("2d")!.putImageData(toImageData(px, pw, ph), 0, 0);
    });
    return () => cancelAnimationFrame(id);
  });

  async function go() {
    const s: ExportSettings = {
      width: size.w,
      height: size.h,
      format,
      frame: reshaped ? frame : undefined,
      captions: burnCaptions,
      thumbnail: thumb ? thumbFrame : null,
      srt,
      quality,
    };
    await exportVideo(s);
    open = false;
  }
</script>

{#if open}
  <div class="modal-bg" role="presentation" onclick={() => !app.exporting && (open = false)}>
    <div class="modal" role="dialog" aria-label="Export video" onclick={(e) => e.stopPropagation()} onkeydown={() => {}} tabindex="-1">
      {#if app.exporting}
        <h2>{app.exporting.label}…</h2>
        <div class="bar"><div style:width="{(app.exporting.done / Math.max(1, app.exporting.total)) * 100}%"></div></div>
        <p class="dim">Frame {app.exporting.done} of {app.exporting.total}. Every frame is rendered exactly, so the video never stutters.</p>
        <div class="btns"><button onclick={cancelExport}>Cancel</button></div>
      {:else}
        <h2>Export video</h2>
        <p class="dim">{p.scenes.length} scenes · {fmtTime(eng().total_frames())} · {p.fps} fps</p>
        <div class="presets">
          {#each PRESETS as pr}
            <button class:on={presetId === pr.id} onclick={() => (presetId = pr.id)}>
              <span class="ico">{pr.icon}</span>
              <span><strong>{pr.label}</strong><small>{pr.hint || `${p.width}×${p.height} as you made it`}</small></span>
            </button>
          {/each}
        </div>
        {#if reshaped}<p class="warn">Your project is {p.width}×{p.height}: this export re-frames it around the middle of the scene, so check that everyone is in shot.</p>{/if}
        <label class="check"><input type="checkbox" bind:checked={burnCaptions} /> Burn in subtitles</label>
        <label class="check"><input type="checkbox" bind:checked={thumb} /> Save a thumbnail</label>
        {#if thumb}
          <div class="thumb">
            <canvas bind:this={canvas} style:width="{previewSize.w}px" style:height="{previewSize.h}px" aria-label="Thumbnail preview"></canvas>
            <div class="pick">
              <input type="range" min="0" max={Math.max(0, totalFrames - 1)} bind:value={thumbFrame} aria-label="Thumbnail frame" />
              <div class="row">
                <span class="dim">{fmtTime(thumbFrame)}</span>
                <button class="link" onclick={() => (thumbFrame = Math.min(playhead, totalFrames - 1))} disabled={thumbFrame === playhead}>Use the playhead</button>
              </div>
            </div>
          </div>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={srt} /> Save subtitles as .srt too</label>
        <div class="opts">
          <button class:on={quality === "high"} onclick={() => (quality = "high")}>Best quality</button>
          <button class:on={quality === "draft"} onclick={() => (quality = "draft")}>Quick draft</button>
        </div>
        <div class="opts">
          <button class:on={format === "mp4"} onclick={() => (format = "mp4")}>MP4 (H.264)</button>
          <button class:on={format === "webm"} onclick={() => (format = "webm")}>WebM (VP9)</button>
        </div>
        <p class="dim">{size.w}×{size.h}{quality === "draft" ? ` at ${Math.round(p.fps / 2)} fps` : ""} · {isDesktop ? "rendered natively with FFmpeg" : "rendered in your browser with WebCodecs"}</p>
        <div class="btns">
          <button onclick={() => (open = false)}>Close</button>
          <button class="primary" onclick={go}>Export</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-bg {
    position: fixed;
    inset: 0;
    background: rgba(8, 9, 16, 0.6);
    backdrop-filter: blur(3px);
    display: grid;
    place-items: center;
    z-index: 50;
  }
  .modal {
    width: 480px;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-radius: 16px;
    padding: 22px;
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5);
  }
  h2 {
    margin: 0 0 6px;
    font-size: 18px;
  }
  .dim {
    color: var(--muted);
    font-size: 13px;
    line-height: 1.5;
  }
  .presets {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    margin: 12px 0 8px;
  }
  .presets button {
    display: flex;
    gap: 9px;
    align-items: center;
    text-align: left;
    padding: 8px 10px;
    border-radius: 10px;
  }
  .presets .on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .presets small {
    display: block;
    color: var(--muted);
    font-size: 11px;
  }
  .ico {
    font-size: 18px;
  }
  .check {
    display: flex;
    gap: 7px;
    align-items: center;
    font-size: 13px;
    margin: 6px 0;
  }
  .thumb {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 2px 0 8px 24px;
  }
  .thumb canvas {
    flex: none;
    border-radius: 8px;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .pick {
    flex: 1;
    min-width: 0;
  }
  .pick input {
    width: 100%;
    accent-color: var(--accent);
  }
  .pick .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .pick .dim {
    font-variant-numeric: tabular-nums;
  }
  .link {
    background: none;
    border: none;
    padding: 2px 0;
    color: var(--accent-2);
    font-size: 12px;
    cursor: pointer;
  }
  .link:disabled {
    color: var(--muted);
    cursor: default;
  }
  .warn {
    font-size: 12px;
    color: #ffcf5a;
    margin: 4px 0;
  }
  .opts {
    display: flex;
    gap: 6px;
    margin: 12px 0 0;
  }
  .opts button {
    flex: 1;
  }
  .opts .on {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  .btns {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .bar {
    height: 10px;
    border-radius: 5px;
    background: var(--bg-3);
    overflow: hidden;
    margin: 14px 0 8px;
  }
  .bar div {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transition: width 0.15s;
  }
</style>
