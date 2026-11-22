<script lang="ts">
  import { app, fmtTime, eng } from "../lib/store.svelte";
  import { exportVideo, cancelExport, isDesktop, type ExportSettings } from "../lib/platform";

  let { open = $bindable(false) }: { open: boolean } = $props();
  const p = $derived(app.project!);
  const presets = $derived([
    { label: "720p", scale: 1280 / Math.max(p.width, p.height) },
    { label: "1080p", scale: 1920 / Math.max(p.width, p.height) },
    { label: "Draft (half size)", scale: 0.5 * (1280 / Math.max(p.width, p.height)) },
  ]);
  let preset = $state(1);
  let format = $state<"mp4" | "webm">("mp4");
  const size = $derived.by(() => {
    const s = presets[preset].scale;
    return { w: Math.round((p.width * s) / 2) * 2, h: Math.round((p.height * s) / 2) * 2 };
  });

  async function go() {
    const s: ExportSettings = { width: size.w, height: size.h, format };
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
        <div class="opts">
          {#each presets as pr, i}
            <button class:on={preset === i} onclick={() => (preset = i)}>{pr.label}</button>
          {/each}
        </div>
        <div class="opts">
          <button class:on={format === "mp4"} onclick={() => (format = "mp4")}>MP4 (H.264)</button>
          <button class:on={format === "webm"} onclick={() => (format = "webm")}>WebM (VP9)</button>
        </div>
        <p class="dim">{size.w}×{size.h} · {isDesktop ? "rendered natively with FFmpeg" : "rendered in your browser with WebCodecs"}</p>
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
    width: 420px;
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
