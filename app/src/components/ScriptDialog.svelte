<script lang="ts">
  // Script to video: paste a dialogue script, get a cast, voices, lip sync,
  // subtitles, music and an edited camera. The engine plans the project
  // (crates/core/src/script.rs); this dialog voices each line with the
  // built-in text-to-speech and places it.
  import { app, eng, loadProject, run, toast } from "../lib/store.svelte";
  import { listVoices, synthBytes, ttsAvailable, termsAccepted, acceptTerms, voiceTerms } from "../lib/voice";
  import { mediaFromBytes } from "../lib/audio";
  import type { Command, Project, SoundInfo } from "../lib/types";

  let { open = $bindable(false) }: { open: boolean } = $props();

  const EXAMPLE = `Title: Lost keys
[kitchen]
Maya (happy, wave): Good morning! Have you seen my keys?
Leo (thinking): Did you check the fridge?
Maya (surprised): The fridge? Why would they be in the fridge!
> Ten minutes later...
[street]
Leo: Found them! They were in your bag.
Maya (happy, cheer): You are the best!`;

  let src = $state(EXAMPLE);
  let shape = $state<"vertical" | "wide" | "square">("vertical");
  let music = $state("music-lofi");
  let captions = $state(true);
  let autoCamera = $state(true);
  let bubbles = $state(false);
  let busy = $state<null | { done: number; total: number; label: string }>(null);
  let cancelled = false;
  let terms = $state<string | null>(null);

  const loops: SoundInfo[] = $derived(app.ready ? (JSON.parse(eng().sound_catalog()) as SoundInfo[]).filter((s) => s.kind === "music") : []);
  const SIZES = { vertical: [1080, 1920], wide: [1280, 720], square: [1080, 1080] } as const;

  /** Built-in voices first, alternating F1, M1, F2, M2... so a two-hander gets two clearly different voices. */
  async function castVoices(): Promise<string[]> {
    if (!ttsAvailable()) return [];
    const all = await listVoices();
    const built = all.filter((v) => v.engine === "supertonic").map((v) => v.id);
    built.sort((a, b) => a.slice(-1).localeCompare(b.slice(-1)) || a.localeCompare(b));
    if (built.length) return built;
    return all.filter((v) => v.language.startsWith("en")).map((v) => v.id);
  }

  async function generate() {
    if (app.dirty && !confirm("Discard unsaved changes in the current project?")) return;
    let voices = await castVoices().catch(() => []);
    if (voices.some((v) => v.startsWith("supertonic:")) && !termsAccepted()) {
      if (terms === null) {
        terms = (await voiceTerms().catch(() => ({ text: "" }))).text || "The built-in voices are licensed under OpenRAIL-M: don't impersonate real people, and label AI speech where you publish it.";
        return;
      }
    }
    cancelled = false;
    const [width, height] = SIZES[shape];
    const opts = { width, height, bubbles: bubbles || !voices.length, voices, captions, music, autoCamera };
    let plan: { project: Project; lines: { scene: string; speaker: string; clipId: string; who: string; text: string; voice: string | null }[]; log: string[] };
    try {
      plan = JSON.parse(eng().script_plan(src, JSON.stringify(opts)));
    } catch (e) {
      toast(String((e as Error).message ?? e), "error");
      return;
    }
    if (!loadProject(JSON.stringify(plan.project), null)) return;
    const failed: string[] = [];
    busy = { done: 0, total: plan.lines.length, label: "Voicing lines" };
    for (const [i, l] of plan.lines.entries()) {
      if (cancelled) break;
      busy = { done: i, total: plan.lines.length, label: `${l.who}: ${l.text.slice(0, 40)}` };
      try {
        const bytes = await synthBytes(l.text, l.voice!, 1);
        const name = l.text.length > 28 ? `${l.text.slice(0, 28)}…` : l.text;
        const media = await mediaFromBytes(bytes, `${name}.wav`, "audio/wav", { kind: "tts", text: l.text, voice: l.voice!, speed: 1 });
        const bubble = app.project?.scenes.find((s) => s.id === l.scene)?.items.find((it) => it.id === l.clipId.replace("clip-", "bubble-"));
        run({ op: "addLine", scene: l.scene, clipId: l.clipId, media, speaker: l.speaker, start: bubble ? bubble.start : null });
      } catch (e) {
        failed.push(`${l.who}: ${String((e as Error).message ?? e)}`);
      }
    }
    // Finish: subtitles, music, camera.
    const cmds: Command[] = [];
    const p = app.project!;
    const voiced = p.scenes.some((s) => s.audio.length);
    if (captions && voiced) cmds.push({ op: "setCaptions", captions: { ...JSON.parse(eng().caption_defaults()), enabled: true } });
    if (music) cmds.push({ op: "addBuiltinMedia", sound: music }, { op: "setMusic", music: { media: `sound-${music}`, volume: 0.3, duck: 0.3, fadeIn: 15, fadeOut: 45, looped: true, start: 0, trimStart: 0 } });
    if (autoCamera && voiced) {
      for (const s of p.scenes) {
        const speakers = new Set(s.audio.map((c) => c.speaker).filter(Boolean));
        if (!speakers.size) continue;
        cmds.push({ op: "autoCamera", scene: s.id, style: speakers.size > 1 || height > width ? "cuts" : "punches" });
      }
    }
    if (cmds.length) run({ op: "batch", commands: cmds });
    busy = null;
    open = false;
    app.sceneIndex = 0;
    app.frame = 0;
    if (failed.length) toast(`${failed.length} line(s) could not be voiced: ${failed[0]}`, "error");
    else toast(voiced ? `Made ${p.scenes.length} scene(s) with ${plan.lines.length} voiced lines. Press Play movie.` : "Made the scenes with speech bubbles (voices need the desktop app).");
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onclick={() => !busy && (open = false)}></div>
  <div class="dialog" role="dialog" aria-label="Script to video">
    <header>
      <h2>Script to video</h2>
      {#if !busy}<button class="x" onclick={() => (open = false)} aria-label="Close">✕</button>{/if}
    </header>
    {#if busy}
      <div class="progress">
        <p>{busy.label}</p>
        <div class="bar"><span style:width="{(busy.done / Math.max(1, busy.total)) * 100}%"></span></div>
        <p class="hint">{busy.done} of {busy.total} lines voiced</p>
        <button onclick={() => (cancelled = true)}>Stop</button>
      </div>
    {:else if terms !== null && !termsAccepted()}
      <div class="terms">
        <p><strong>Built-in voices</strong></p>
        <pre>{terms}</pre>
        <button class="primary" onclick={() => { acceptTerms(); terms = null; generate(); }}>I agree, make the video</button>
        <button onclick={() => (terms = null)}>Back</button>
      </div>
    {:else}
      <div class="cols">
        <label class="script">
          <span>Script</span>
          <textarea bind:value={src} spellcheck="false" rows="14"></textarea>
        </label>
        <div class="side">
          <div class="field"><span>Shape</span>
            <div class="seg">
              <button class:on={shape === "vertical"} onclick={() => (shape = "vertical")}>9:16</button>
              <button class:on={shape === "wide"} onclick={() => (shape = "wide")}>16:9</button>
              <button class:on={shape === "square"} onclick={() => (shape = "square")}>1:1</button>
            </div>
          </div>
          <label class="field"><span>Music</span>
            <select bind:value={music}>
              <option value="">None</option>
              {#each loops as l}<option value={l.id}>{l.name} ({l.category})</option>{/each}
            </select>
          </label>
          <label class="check"><input type="checkbox" bind:checked={captions} /> Subtitles</label>
          <label class="check"><input type="checkbox" bind:checked={autoCamera} /> Camera follows the speaker</label>
          <label class="check"><input type="checkbox" bind:checked={bubbles} /> Speech bubbles too</label>
          <div class="help">
            <p><code>[kitchen]</code> starts a scene (any background id; <code>[park, 6s]</code> sets a length)</p>
            <p><code>Name (happy, wave): line</code> is dialogue; brackets pick an expression and an action</p>
            <p><code>&gt; text</code> is an on-screen caption; <code>Style: stick</code> casts stick figures</p>
          </div>
          {#if !ttsAvailable()}<p class="hint">Voices need the desktop app; here the lines become speech bubbles.</p>{/if}
        </div>
      </div>
      <footer>
        <button onclick={() => (src = EXAMPLE)}>Example</button>
        <button class="primary" onclick={generate} disabled={!src.trim()}>Make the video</button>
      </footer>
    {/if}
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
    width: min(860px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-radius: 16px;
    padding: 18px 20px 16px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  footer {
    justify-content: flex-end;
    margin-top: 12px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 17px;
  }
  .x {
    background: none;
    border: none;
    color: var(--muted);
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 260px;
    gap: 16px;
  }
  .script {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  textarea {
    font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
    font-size: 12.5px;
    line-height: 1.5;
    min-height: 300px;
    resize: vertical;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    color: var(--muted);
  }
  .seg {
    display: flex;
    gap: 4px;
  }
  .seg button {
    flex: 1;
  }
  .seg .on {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  .check {
    font-size: 13px;
    display: flex;
    gap: 7px;
    align-items: center;
  }
  .help {
    font-size: 11.5px;
    color: var(--muted);
    line-height: 1.4;
    border-top: 1px solid var(--line);
    padding-top: 8px;
  }
  .help p {
    margin: 0 0 6px;
  }
  code {
    color: var(--text);
  }
  .hint {
    font-size: 12px;
    color: var(--muted);
  }
  .progress {
    padding: 20px 0;
  }
  .bar {
    height: 8px;
    border-radius: 4px;
    background: var(--bg-3);
    overflow: hidden;
    margin: 10px 0;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .terms pre {
    white-space: pre-wrap;
    font-size: 12px;
    max-height: 300px;
    overflow: auto;
    background: var(--bg-2);
    padding: 10px;
    border-radius: 8px;
  }
</style>
