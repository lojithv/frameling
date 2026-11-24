<script lang="ts">
  // "New from template": pick a ready-made project and a frame shape.
  import { app, eng, loadProject, toast } from "../lib/store.svelte";
  import { toImageData } from "../lib/thumbs";

  let { open = $bindable(false) }: { open: boolean } = $props();

  interface TemplateInfo { id: string; name: string; description: string; best: "vertical" | "wide" | "square" }
  const SHAPES = [
    { id: "vertical", label: "Vertical 9:16", hint: "Shorts, Reels, TikTok", w: 54, h: 96 },
    { id: "wide", label: "Wide 16:9", hint: "YouTube", w: 128, h: 72 },
    { id: "square", label: "Square 1:1", hint: "Posts", w: 96, h: 96 },
  ] as const;
  type Shape = (typeof SHAPES)[number]["id"];

  let shape = $state<Shape>("vertical");
  const templates: TemplateInfo[] = $derived(app.ready ? JSON.parse(eng().templates()) : []);
  const thumbs = new Map<string, string>();

  function thumb(id: string, s: Shape): string {
    const key = `${id}-${s}`;
    const hit = thumbs.get(key);
    if (hit) return hit;
    const sh = SHAPES.find((x) => x.id === s)!;
    const k = 2.2;
    const w = Math.round(sh.w * k) & ~1;
    const h = Math.round(sh.h * k) & ~1;
    const px = eng().template_thumb(id, s, w, h);
    const c = document.createElement("canvas");
    c.width = w;
    c.height = h;
    c.getContext("2d")!.putImageData(toImageData(px, w, h), 0, 0);
    const url = c.toDataURL();
    thumbs.set(key, url);
    return url;
  }

  function create(t: TemplateInfo) {
    if (app.dirty && !confirm("Discard unsaved changes?")) return;
    try {
      eng().load_template(t.id, shape);
      if (loadProject(eng().project_json(), null)) {
        open = false;
        toast(`${t.name}: select a character and type a line in Voice, or paste a whole script with Script to video.`);
      }
    } catch (e) {
      toast(String((e as Error).message ?? e), "error");
    }
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onclick={() => (open = false)}></div>
  <div class="dialog" role="dialog" aria-label="New from template">
    <header>
      <h2>New from template</h2>
      <button class="x" onclick={() => (open = false)} aria-label="Close">✕</button>
    </header>
    <div class="shapes" role="radiogroup" aria-label="Frame shape">
      {#each SHAPES as s}
        <button class:on={shape === s.id} onclick={() => (shape = s.id)} role="radio" aria-checked={shape === s.id}>
          <span class="frame" style:width="{s.w / 4}px" style:height="{s.h / 4}px"></span>
          <span><strong>{s.label}</strong><small>{s.hint}</small></span>
        </button>
      {/each}
    </div>
    <div class="grid" class:tall={shape === "vertical"}>
      {#each templates as t (t.id)}
        <button class="card" onclick={() => create(t)}>
          {#key shape}<img alt="" src={thumb(t.id, shape)} />{/key}
          <span class="name">{t.name}{#if t.best === shape}<em>best fit</em>{/if}</span>
          <span class="desc">{t.description}</span>
        </button>
      {/each}
    </div>
    <p class="hint">Every template comes with music, subtitles and a voice for each character. Built-in music is original and free to use.</p>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(5, 6, 12, 0.6);
    z-index: 40;
  }
  .dialog {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(940px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-radius: 16px;
    padding: 18px 20px 16px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  .x {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 15px;
  }
  .shapes {
    display: flex;
    gap: 8px;
    margin: 14px 0;
  }
  .shapes button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    border-radius: 10px;
    text-align: left;
  }
  .shapes button.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .shapes small {
    display: block;
    color: var(--muted);
    font-size: 11px;
  }
  .frame {
    border: 2px solid currentColor;
    border-radius: 3px;
    opacity: 0.8;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }
  .grid.tall {
    grid-template-columns: repeat(6, 1fr);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-radius: 12px;
    text-align: left;
    background: var(--bg-2);
    transition: transform 0.12s, border-color 0.12s;
  }
  .card:hover {
    border-color: var(--accent);
    transform: translateY(-2px);
  }
  .card img {
    width: 100%;
    border-radius: 8px;
    display: block;
    background: var(--bg-3);
  }
  .name {
    font-weight: 600;
    font-size: 13px;
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .name em {
    font-style: normal;
    font-size: 10px;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 0 6px;
  }
  .desc {
    font-size: 11.5px;
    color: var(--muted);
    line-height: 1.35;
  }
  .hint {
    color: var(--muted);
    font-size: 12px;
    margin: 12px 0 0;
  }
</style>
