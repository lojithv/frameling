<script lang="ts">
  // GoAnimate-style "make it talk": pick a voice for the character, type a
  // line (built-in TTS), record one, or upload one. Lines go on the scene one
  // after another and the scene grows to fit.
  import { app, currentScene, run, secs, frames, toast, updateItem } from "../lib/store.svelte";
  import { listVoices, previewVoice, sayLine, redoLine, startRecording, stopRecording, addRecordedLine, ttsAvailable, termsAccepted, acceptTerms, voiceTerms, splitVoiceId, BUILT_IN_LANGS } from "../lib/voice";
  import { importAudio, previewMedia, stopAudio } from "../lib/audio";
  import { placeLine } from "../lib/voice";
  import type { CharacterKind, Item, Voice } from "../lib/types";

  let { item }: { item: Item } = $props();
  const ch = $derived(item.kind as CharacterKind);
  const scene = $derived(currentScene());
  const lines = $derived(scene?.audio.filter((c) => c.speaker === item.id).sort((a, b) => a.start - b.start) ?? []);

  let voices = $state<Voice[]>([]);
  let loadingVoices = $state(true);
  let text = $state("");
  let busy = $state<null | "say" | "rec" | "upload" | "preview" | string>(null);
  let recording = $state(false);

  $effect(() => {
    listVoices()
      .then((v) => (voices = v))
      .catch((e) => toast(`Voices unavailable: ${e}`, "error"))
      .finally(() => (loadingVoices = false));
  });

  const voiceId = $derived(ch.voice ?? voices[0]?.id ?? "");
  // Built-in voices carry their language as a suffix: `supertonic:F1@de`.
  const voiceBase = $derived(splitVoiceId(voiceId).base);
  const voiceLang = $derived(splitVoiceId(voiceId).lang ?? "en");
  const isBuiltIn = $derived(voiceBase.startsWith("supertonic:"));
  const voiceName = (id: string) => {
    const { base, lang } = splitVoiceId(id);
    const name = voices.find((v) => v.id === base)?.name ?? base.replace(/^\w+:/, "");
    return lang && lang !== "en" ? `${name} · ${lang.toUpperCase()}` : name;
  };

  // OpenRAIL-M terms for the built-in voices, accepted once.
  let accepted = $state(termsAccepted());
  let terms = $state<{ text: string; licence: string | null } | null>(null);
  $effect(() => {
    if (isBuiltIn && !accepted && !terms) voiceTerms().then((t) => (terms = t)).catch(() => {});
  });

  const groups = $derived.by(() => {
    const g = new Map<string, Voice[]>();
    for (const v of voices) {
      const key = v.engine !== "supertonic" ? "System voices" : v.id.startsWith("supertonic:F") ? "Built-in voices · female" : "Built-in voices · male";
      if (!g.has(key)) g.set(key, []);
      g.get(key)!.push(v);
    }
    return [...g.entries()];
  });

  /** Pick a voice from the list; a built-in voice keeps the character's language. */
  function pickVoice(base: string) {
    setVoice(base.startsWith("supertonic:") && voiceLang !== "en" ? `${base}@${voiceLang}` : base);
  }
  function setLang(lang: string) {
    setVoice(lang === "en" ? voiceBase : `${voiceBase}@${lang}`);
  }
  function setVoice(id: string) {
    const it = structuredClone($state.snapshot(item)) as Item;
    (it.kind as CharacterKind).voice = id;
    updateItem(it);
  }
  function setSpeed(v: number) {
    const it = structuredClone($state.snapshot(item)) as Item;
    (it.kind as CharacterKind).voiceSpeed = v;
    updateItem(it, "vspeed");
  }

  async function say() {
    if (!text.trim() || !voiceId) return;
    busy = "say";
    try {
      if (await sayLine(text.trim(), voiceId, ch.voiceSpeed ?? 1, item.id)) text = "";
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }

  async function toggleRecord() {
    try {
      if (!recording) {
        await startRecording();
        recording = true;
      } else {
        recording = false;
        busy = "rec";
        const wav = await stopRecording();
        await addRecordedLine(wav, item.id, item.name);
      }
    } catch (e) {
      recording = false;
      toast(`Recording failed: ${(e as Error).message ?? e}`, "error");
    } finally {
      if (!recording) busy = null;
    }
  }

  async function upload(e: Event) {
    const input = e.target as HTMLInputElement;
    const f = input.files?.[0];
    input.value = "";
    if (!f) return;
    busy = "upload";
    try {
      placeLine(await importAudio(f), item.id);
    } catch (err) {
      toast(`Could not read that audio file (${(err as Error).message})`, "error");
    } finally {
      busy = null;
    }
  }

  function remove(id: string) {
    if (scene) run({ op: "removeAudio", scene: scene.id, id });
  }
  function nudge(id: string, deltaSecs: number) {
    const c = lines.find((l) => l.id === id);
    if (!c || !scene) return;
    run({ op: "updateAudio", scene: scene.id, clip: { ...$state.snapshot(c), start: Math.max(0, c.start + frames(deltaSecs)) } });
  }
  async function revoice(id: string) {
    const c = lines.find((l) => l.id === id);
    const src = c && app.project?.media[c.media]?.source;
    if (!c || src?.kind !== "tts") return;
    busy = id;
    try {
      await redoLine(id, src.text, voiceId, ch.voiceSpeed ?? 1);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
</script>

<section>
  <h3>Voice</h3>
  {#if ttsAvailable()}
    {#if loadingVoices}
      <p class="hint">Loading voices…</p>
    {:else if voices.length === 0}
      <p class="hint">No voices found. Run <code>npm run fetch-tts</code> before building, or install system voices.</p>
    {:else}
      <div class="voice-row">
        <select value={voiceBase} onchange={(e) => pickVoice((e.target as HTMLSelectElement).value)} aria-label="Voice">
          {#each groups as [label, list]}
            <optgroup {label}>
              {#each list as v}<option value={v.id}>{v.engine === "supertonic" ? v.name : `${v.name} · ${v.language}`}</option>{/each}
            </optgroup>
          {/each}
        </select>
        {#if isBuiltIn}
          <select class="lang" value={voiceLang} onchange={(e) => setLang((e.target as HTMLSelectElement).value)} aria-label="Language" title="Language this character speaks">
            {#each BUILT_IN_LANGS as [code, name]}<option value={code}>{name}</option>{/each}
          </select>
        {/if}
        <button title="Hear this voice" aria-label="Hear this voice" disabled={busy !== null || (isBuiltIn && !accepted)} onclick={async () => { busy = "preview"; await previewVoice(voiceId, ch.voiceSpeed ?? 1, `Hi, I'm ${item.name}.`); busy = null; }}>{busy === "preview" ? "…" : "▶"}</button>
      </div>
      {#if isBuiltIn && !accepted}
        <div class="terms" role="region" aria-label="Built-in voice terms">
          <p>{terms?.text ?? "The built-in voices come with usage terms."}</p>
          {#if terms?.licence}<p class="dim">Full licence: <code>{terms.licence}</code></p>{/if}
          <button class="primary wide" onclick={() => { acceptTerms(); accepted = true; }}>I agree, use the built-in voices</button>
        </div>
      {/if}
      <label class="field slider">
        <span>Speed</span>
        <input type="range" min="0.6" max="1.6" step="0.05" value={ch.voiceSpeed ?? 1} oninput={(e) => setSpeed(+(e.target as HTMLInputElement).value)} />
        <output>{(ch.voiceSpeed ?? 1).toFixed(2)}×</output>
      </label>
      {@const credit = voices.find((v) => v.id === voiceId)?.credit}
      {#if credit}<p class="credit">{credit}</p>{/if}
      <textarea rows="2" placeholder="What should {item.name} say?" bind:value={text}
        onkeydown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) say(); }}></textarea>
      <button class="primary wide" disabled={!text.trim() || busy !== null || (isBuiltIn && !accepted)} onclick={say}>{busy === "say" ? "Speaking…" : "＋ Add line"}</button>
    {/if}
  {:else}
    <p class="hint">Typed lines use the built-in voices in the desktop app. Here you can record or upload.</p>
  {/if}
  <div class="row2 gap">
    <button class:rec={recording} disabled={busy !== null && !recording} onclick={toggleRecord}>{recording ? "■ Stop" : busy === "rec" ? "Saving…" : "● Record"}</button>
    <label class="btn" class:disabled={busy !== null}>
      {busy === "upload" ? "Reading…" : "⇪ Upload"}
      <input type="file" accept="audio/*" onchange={upload} disabled={busy !== null} />
    </label>
  </div>

  {#if lines.length}
    <h4>Lines in this scene</h4>
    {#each lines as c (c.id)}
      {@const m = app.project?.media[c.media]}
      <div class="line">
        <div class="txt" title={m?.source?.kind === "tts" ? m.source.text : m?.name}>
          {m?.source?.kind === "tts" ? `“${m.source.text}”` : m?.source?.kind === "recording" ? "🎙 Recording" : `♪ ${m?.name ?? "missing"}`}
        </div>
        <div class="meta">
          <span>{secs(c.start).toFixed(1)}s · {((m?.durationFrames ?? 0) / (app.project?.fps ?? 30)).toFixed(1)}s</span>
          {#if m?.source?.kind === "tts"}<span class="dim">{voiceName(m.source.voice)}</span>{/if}
        </div>
        <div class="tools">
          <button title="Play" onclick={() => previewMedia(c.media)}>▶</button>
          <button title="Earlier" onclick={() => nudge(c.id, -0.25)}>◀</button>
          <button title="Later" onclick={() => nudge(c.id, 0.25)}>▶▶</button>
          {#if m?.source?.kind === "tts" && ttsAvailable()}
            <button title="Re-speak with the current voice and speed" disabled={busy !== null} onclick={() => revoice(c.id)}>{busy === c.id ? "…" : "↻"}</button>
          {/if}
          <button title="Jump to line" onclick={() => { stopAudio(); app.frame = c.start; }}>⤓</button>
          <button class="x" title="Delete line" onclick={() => remove(c.id)}>✕</button>
        </div>
      </div>
    {/each}
  {/if}
</section>

<style>
  section { border-top: 1px solid var(--line); padding: 12px 0 6px; }
  h3 { margin: 0 0 10px; font-size: 11px; text-transform: uppercase; letter-spacing: 0.07em; color: var(--muted); }
  h4 { margin: 14px 0 6px; font-size: 12px; color: var(--muted); font-weight: 500; }
  .hint { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 0 0 8px; }
  .credit { color: var(--muted); font-size: 11px; margin: -4px 0 8px; }
  .voice-row { display: flex; gap: 6px; margin-bottom: 8px; }
  .voice-row select { flex: 1; min-width: 0; }
  .voice-row select.lang { flex: 0 0 88px; }
  .terms { background: var(--bg-2); border: 1px solid var(--line); border-radius: 10px; padding: 10px; margin-bottom: 8px; font-size: 11.5px; line-height: 1.45; }
  .terms p { margin: 0 0 8px; }
  .terms code { word-break: break-all; }
  .field.slider { display: grid; grid-template-columns: 74px 1fr 52px; align-items: center; gap: 8px; font-size: 12px; color: var(--muted); margin-bottom: 8px; }
  .field.slider output { text-align: right; color: var(--text); }
  textarea { width: 100%; resize: vertical; margin-bottom: 6px; }
  .wide { width: 100%; }
  .row2 { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .gap { margin-top: 8px; }
  .btn { display: grid; place-items: center; background: var(--bg-2); border: 1px solid var(--line); border-radius: 8px; padding: 7px 12px; cursor: pointer; font-size: 14px; }
  .btn input { display: none; }
  .btn.disabled { opacity: 0.4; pointer-events: none; }
  .rec { border-color: #e84a5f; color: #ff8a98; animation: pulse 1s infinite alternate; }
  @keyframes pulse { to { background: rgba(232, 74, 95, 0.18); } }
  .line { background: var(--bg-2); border: 1px solid var(--line); border-radius: 10px; padding: 8px 10px; margin-bottom: 6px; }
  .txt { font-size: 12.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { display: flex; justify-content: space-between; font-size: 11px; color: var(--muted); margin: 3px 0 6px; }
  .tools { display: flex; gap: 4px; }
  .tools button { padding: 2px 7px; font-size: 11px; }
  .tools .x { margin-left: auto; }
  .dim { color: var(--muted); }
  code { font-size: 11px; }
</style>
