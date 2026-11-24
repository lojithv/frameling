<script lang="ts">
  import { app, currentScene, selectedPosed, selectedItem, posedItem, positionKeyFrames, setPositionKey, toast, editItem, keyFrames, setAutoKey, updateScene, endGesture, run, secs, frames, fmtTime } from "../lib/store.svelte";
  import { EXPRESSIONS, PALETTE, SKIN_TONES, uid } from "../lib/defaults";
  import VoicePanel from "./VoicePanel.svelte";
  import PosePanel from "./PosePanel.svelte";
  import type { LipSync, MouthSet, Viseme } from "../lib/types";
  import type { AudioClip, CaptionStyle, Captions, CharacterKind, Effect, Emphasis, Font, Item, PropKind, Reveal, Scene, TextKind, TransitionKind, View } from "../lib/types";
  import { saveSubtitles } from "../lib/platform";
  import { eng } from "../lib/store.svelte";

  const scene = $derived(currentScene());
  const item = $derived(selectedPosed());
  const project = $derived(app.project);

  const snap = <T,>(v: T): T => structuredClone($state.snapshot(v)) as T;

  /** Edit the selected item. `key` merges a slider drag into one undo step. */
  function edit(fn: (it: Item) => void, key?: string) {
    if (!item) return;
    const it = snap(item);
    fn(it);
    editItem(it, key);
  }
  function editKind<K extends Item["kind"]>(fn: (k: K) => void, key?: string) {
    edit((it) => fn(it.kind as K), key);
  }
  function editScene(fn: (s: Scene) => void, key?: string) {
    if (!scene) return;
    const s = snap(scene);
    fn(s);
    updateScene(s, key);
  }

  function duplicate() {
    if (!item || !scene) return;
    const copy = snap(item);
    copy.id = uid(item.kind.type);
    copy.x += 40;
    copy.y += 0;
    if (copy.motion) copy.motion.x += 40;
    const idx = scene.items.findIndex((i) => i.id === item.id);
    if (run({ op: "addItem", scene: scene.id, item: copy, index: idx + 1 })) app.selected = copy.id;
  }
  function remove() {
    if (item && scene) run({ op: "removeItem", scene: scene.id, id: item.id });
  }
  function order(delta: number) {
    if (!item || !scene) return;
    const idx = scene.items.findIndex((i) => i.id === item.id);
    run({ op: "moveItem", scene: scene.id, id: item.id, to: Math.max(0, idx + delta) });
  }

  const effects: { id: Effect; label: string }[] = [
    { id: "none", label: "None" },
    { id: "fade", label: "Fade" },
    { id: "pop", label: "Pop" },
    { id: "slideLeft", label: "Slide from left" },
    { id: "slideRight", label: "Slide from right" },
    { id: "slideUp", label: "Slide up" },
    { id: "slideDown", label: "Slide down" },
    { id: "bounce", label: "Bounce" },
    { id: "zoom", label: "Zoom" },
    { id: "spin", label: "Spin" },
  ];
  const FONTS: { id: Font; label: string }[] = [
    { id: "sans", label: "Sans" }, { id: "heavy", label: "Heavy" }, { id: "rounded", label: "Rounded" },
    { id: "mono", label: "Mono" }, { id: "serif", label: "Serif" },
  ];
  const REVEALS: { id: Reveal; label: string }[] = [
    { id: "none", label: "All at once" },
    { id: "typewriter", label: "Typewriter" },
    { id: "popLetters", label: "Pop letters" },
    { id: "popWords", label: "Pop words" },
    { id: "riseWords", label: "Rise words" },
    { id: "bounce", label: "Bounce letters" },
    { id: "fade", label: "Fade letters" },
  ];
  const EMPHASES: { id: Emphasis; label: string }[] = [
    { id: "none", label: "None" }, { id: "wave", label: "Wave" }, { id: "pulse", label: "Pulse" }, { id: "shake", label: "Shake" },
  ];
  const CAPTION_STYLES: { id: CaptionStyle; label: string; hint: string }[] = [
    { id: "pop", label: "Pop", hint: "Outlined; the spoken word changes colour and pops" },
    { id: "boxed", label: "Boxed", hint: "On a dark panel; the spoken word changes colour" },
    { id: "pill", label: "Pill", hint: "The spoken word sits on a coloured pill" },
    { id: "karaoke", label: "Karaoke", hint: "Words fill with colour as they are spoken" },
    { id: "clean", label: "Clean", hint: "Plain outlined text, no highlight" },
  ];
  /** Frame positions for pinned text: stage y as seen with the camera at home. */
  const PIN_SPOTS = [{ y: 110, label: "Top" }, { y: 360, label: "Middle" }, { y: 560, label: "Lower" }, { y: 650, label: "Bottom" }];

  // ------------------------------------------------------------ captions
  const captions = $derived<Captions | null>(project?.captions ?? (app.ready ? JSON.parse(eng().caption_defaults()) : null));
  function editCaptions(fn: (c: Captions) => void, key?: string) {
    if (!captions) return;
    const c = structuredClone($state.snapshot(captions)) as Captions;
    fn(c);
    run({ op: "setCaptions", captions: c }, key);
  }
  function toggleCaptions() {
    // First time on: start from the defaults for this frame shape.
    const base = project?.captions ? (structuredClone($state.snapshot(project.captions)) as Captions) : (JSON.parse(eng().caption_defaults()) as Captions);
    base.enabled = !base.enabled;
    run({ op: "setCaptions", captions: base });
  }
  const isVoiceLine = (c: AudioClip) => project?.media[c.media]?.source?.kind === "tts";
  const lineText = (c: AudioClip) => {
    const s = project?.media[c.media]?.source;
    return s?.kind === "tts" ? s.text : "";
  };
  const transitions: { id: TransitionKind; label: string }[] = [
    { id: "cut", label: "Cut" },
    { id: "fade", label: "Crossfade" },
    { id: "fadeBlack", label: "Dip to black" },
    { id: "slide", label: "Slide" },
    { id: "zoom", label: "Zoom" },
  ];
  const sizes = [
    { w: 1280, h: 720, label: "HD 16:9 (1280×720)" },
    { w: 1920, h: 1080, label: "Full HD 16:9 (1920×1080)" },
    { w: 1080, h: 1920, label: "Vertical 9:16 (1080×1920)" },
    { w: 1080, h: 1080, label: "Square (1080×1080)" },
  ];

  const propEntry = $derived(
    item?.kind.type === "prop" ? app.catalog?.props.find((p) => p.id === (item!.kind as PropKind).prop) : null,
  );
  const bgEntry = $derived(app.catalog?.backgrounds.find((b) => b.id === scene?.background.preset));
  const characters = $derived(scene?.items.filter((i) => i.kind.type === "character") ?? []);
  const speech = $derived(scene?.audio.filter((c) => item && c.speaker === item.id) ?? []);

  function editClip(c: AudioClip, fn: (c: AudioClip) => void, key?: string) {
    if (!scene) return;
    const n = snap(c);
    fn(n);
    run({ op: "updateAudio", scene: scene.id, clip: n }, key);
  }
  const fps = $derived(project?.fps ?? 30);
  // Facing, laid out as seen from above the stage: the camera is at the bottom.
  const VIEWS: { id: View; label: string; icon: string; area: string }[] = [
    { id: "back", label: "Back (facing away)", icon: "⬆", area: "n" },
    { id: "backRight", label: "Back, turned right", icon: "↗", area: "ne" },
    { id: "right", label: "Facing right (side)", icon: "➡", area: "e" },
    { id: "frontRight", label: "Front, turned right (3/4)", icon: "↘", area: "se" },
    { id: "front", label: "Front", icon: "⬇", area: "s" },
    { id: "frontLeft", label: "Front, turned left (3/4)", icon: "↙", area: "sw" },
    { id: "left", label: "Facing left (side)", icon: "⬅", area: "w" },
    { id: "backLeft", label: "Back, turned left", icon: "↖", area: "nw" },
  ];

  // ------------------------------------------------------------ keyframes
  const keys = $derived(item ? keyFrames(item) : []);
  const onKey = $derived(keys.includes(app.frame));
  /** Plain-language names of the properties keyed on this frame. */
  const PROP_NAMES: Record<string, string> = {
    "transform.x": "position", "transform.y": "position", "transform.scale": "size", "transform.rotation": "rotation",
    "transform.opacity": "opacity", "transform.flip": "flip", expression: "expression", outfit: "outfit",
    "look.skin": "skin", "look.hair": "hair colour", "look.hairStyle": "hair style", "look.top": "top",
    "look.bottom": "bottom", "look.shoes": "shoes", "look.accent": "accent", "look.build": "build",
    "look.accessory": "accessory", "look.style": "style", "look.dress": "dress", mouth: "mouth",
  };
  // ------------------------------------------------------------ lip sync
  const VISEMES: { id: Viseme | "auto"; label: string; hint: string }[] = [
    { id: "auto", label: "Auto", hint: "Follow the voice line" },
    { id: "rest", label: "Rest", hint: "Closed, the expression's own mouth" },
    { id: "mbp", label: "M B P", hint: "Lips pressed together" },
    { id: "ee", label: "EE", hint: "Teeth together: S, T, K, N, EE" },
    { id: "eh", label: "EH", hint: "Open: E as in bed" },
    { id: "ai", label: "AH", hint: "Wide open: A as in father" },
    { id: "o", label: "O", hint: "Round: O as in go" },
    { id: "u", label: "OO", hint: "Pursed: OO, W, Q" },
    { id: "fv", label: "F V", hint: "Top teeth on the lower lip" },
    { id: "l", label: "L TH", hint: "Tongue up behind the teeth" },
  ];
  /** The mouth shape keyed at the playhead ("auto" when none). */
  const mouthHere = $derived.by(() => {
    const keys = item?.tracks?.mouth?.keys ?? [];
    let v: unknown = "auto";
    for (const k of keys) if (k.frame <= app.frame) v = k.value;
    return v as Viseme | "auto";
  });
  const mouthKeys = $derived(item?.tracks?.mouth?.keys.length ?? 0);
  /** Key a mouth shape at the playhead; it holds until the next mouth key. */
  function keyMouth(v: Viseme | "auto") {
    if (!scene || !item) return;
    const cmds: import("../lib/types").Command[] = [{ op: "setKeys", scene: scene.id, item: item.id, path: "mouth", keys: [{ frame: app.frame, value: v, interp: "constant" }] }];
    // A one-off shape: go back to automatic on the next frame unless keyed there.
    const next = item.tracks?.mouth?.keys.find((k) => k.frame === app.frame + 1);
    if (v !== "auto" && !next && scene.duration > app.frame + 1) cmds.push({ op: "setKeys", scene: scene.id, item: item.id, path: "mouth", keys: [{ frame: app.frame + 1, value: "auto", interp: "constant" }] });
    run({ op: "batch", commands: cmds });
  }
  function holdMouth(v: Viseme) {
    if (!scene || !item) return;
    run({ op: "setKeys", scene: scene.id, item: item.id, path: "mouth", keys: [{ frame: app.frame, value: v, interp: "constant" }] });
  }
  const keyedHere = $derived.by(() => {
    if (!item?.tracks) return [];
    const names = Object.entries(item.tracks)
      .filter(([, t]) => t.keys.some((k) => k.frame === app.frame))
      .map(([p]) => PROP_NAMES[p] ?? p.replace(/^bone\./, ""));
    return [...new Set(names)];
  });
  const prevKey = $derived([...keys].reverse().find((f) => f < app.frame));
  const nextKey = $derived(keys.find((f) => f > app.frame));
  // ------------------------------------------------------------ movement
  const moves = $derived.by(() => {
    void app.rev;
    const raw = selectedItem();
    if (!raw) return [];
    const fs = positionKeyFrames(raw);
    return fs.map((f, i) => {
      const p = posedItem(raw, f);
      return { f, x: Math.round(p.x), y: Math.round(p.y), label: i === 0 ? "Start" : i === fs.length - 1 ? "End" : `Point ${i + 1}` };
    });
  });
  const POS = ["transform.x", "transform.y"];
  function retime(from: number, secsValue: number) {
    if (!scene || !item) return;
    const to = Math.max(0, Math.min(scene.duration - 1, frames(secsValue)));
    if (to === from) return;
    if (moves.some((m) => m.f === to)) return toast(`There is already a movement point at ${fmtTime(to)}`, "error");
    run({ op: "moveKeys", scene: scene.id, item: item.id, from, to, paths: POS.filter((p) => selectedItem()?.tracks?.[p]?.keys.some((k) => k.frame === from)) });
  }
  function setXY(f: number, x: number, y: number) {
    const raw = selectedItem();
    if (raw) setPositionKey(raw, f, x, y);
  }
  function removeMove(f: number) {
    if (!scene || !item) return;
    const raw = selectedItem();
    run({ op: "batch", commands: POS.filter((p) => raw?.tracks?.[p]?.keys.some((k) => k.frame === f)).map((path) => ({ op: "deleteKeys" as const, scene: scene!.id, item: item!.id, path, frames: [f] })) });
  }
  function addMoveHere() {
    const raw = selectedItem();
    if (!raw) return;
    const p = posedItem(raw, app.frame);
    setPositionKey(raw, app.frame, p.x, p.y);
  }

  function deleteKeysHere() {
    if (item && scene) run({ op: "deleteKeys", scene: scene.id, item: item.id, frames: [app.frame] });
  }
  function clearKeys() {
    if (item && scene && confirm(`Remove all ${keys.length} keyframes from ${item.name}? It keeps how it looks at the playhead.`)) {
      // Bake the current pose into the plain fields, then drop every track.
      const it = snap(item);
      it.tracks = {};
      run({ op: "updateItem", scene: scene.id, item: it });
    }
  }
</script>

{#snippet swatches(value: string, colors: string[], onpick: (c: string) => void)}
  <div class="swatches">
    {#each colors as c}
      <button class="sw" class:on={value.slice(0, 7).toLowerCase() === c.toLowerCase()} style:background={c} title={c} aria-label={c} onclick={() => onpick(c)}></button>
    {/each}
    <label class="sw custom" title="Custom colour">
      <input type="color" value={value.slice(0, 7)} oninput={(e) => onpick((e.target as HTMLInputElement).value)} />
    </label>
  </div>
{/snippet}

{#snippet seg<T extends string>(value: T, options: { id: T; label: string }[], onpick: (v: T) => void)}
  <div class="seg">
    {#each options as o}
      <button class:on={value === o.id} onclick={() => onpick(o.id)}>{o.label}</button>
    {/each}
  </div>
{/snippet}

{#snippet slider(label: string, value: number, min: number, max: number, step: number, key: string, onset: (v: number) => void, fmt?: (v: number) => string)}
  <label class="field slider">
    <span>{label}</span>
    <input type="range" {min} {max} {step} {value} oninput={(e) => onset(+(e.target as HTMLInputElement).value)} onchange={endGesture} data-key={key} />
    <output>{fmt ? fmt(value) : value}</output>
  </label>
{/snippet}

<aside class="panel inspector">
  {#if item && scene}
    {@const kind = item.kind}
    <header>
      <input class="name" value={item.name} onchange={(e) => edit((it) => (it.name = (e.target as HTMLInputElement).value))} />
      <span class="badge {kind.type}">{kind.type}</span>
    </header>
    <div class="actions">
      <button onclick={duplicate} title="Duplicate (Ctrl+D)">⧉ Duplicate</button>
      <button onclick={() => order(1)} title="Bring forward">⬆</button>
      <button onclick={() => order(-1)} title="Send backward">⬇</button>
      <button onclick={() => edit((it) => (it.flip = !it.flip))} title="Flip horizontally" class:on={item.flip}>⇋</button>
      <button onclick={() => edit((it) => (it.locked = !it.locked))} title="Lock position" class:on={item.locked}>{item.locked ? "🔒" : "🔓"}</button>
      <button class="danger" onclick={remove} title="Delete (Del)">🗑</button>
    </div>
    <div class="keys">
      <button class="auto" class:on={app.autoKey} onclick={() => setAutoKey(!app.autoKey)}
        title="Auto-key: when on, changing anything after the item's first frame records a keyframe at the playhead">
        <span class="dot"></span>Auto-key
      </button>
      <button onclick={() => prevKey !== undefined && (app.frame = prevKey)} disabled={prevKey === undefined} title="Previous keyframe">◀◆</button>
      <span class="diamond" class:on={onKey} title={onKey ? `Keyframe here: ${keyedHere.join(", ")}` : "No keyframe at the playhead"}>◆</span>
      <button onclick={() => nextKey !== undefined && (app.frame = nextKey)} disabled={nextKey === undefined} title="Next keyframe">◆▶</button>
      <button onclick={deleteKeysHere} disabled={!onKey} title="Delete the keyframes at the playhead">✕◆</button>
      {#if keys.length}<button onclick={clearKeys} title="Remove all keyframes from this item">Clear</button>{/if}
    </div>
    <p class="keyhint">
      {#if onKey}Keyframe at {fmtTime(app.frame)}: {keyedHere.join(", ")}.
      {:else if keys.length}{keys.length} keyframe{keys.length === 1 ? "" : "s"}. Change something here to add one at {fmtTime(app.frame)}.
      {:else if app.autoKey}Move the playhead ahead and change position, size, rotation, expression or colours to animate.
      {:else}Auto-key is off: changes set the item's look for the whole scene.{/if}
    </p>
    {#if moves.length >= 2}
      <section class="moves">
        <h3>Movement</h3>
        <p class="keyhint">The {item.kind.type === "character" ? "character stands still" : "item stays"} before the start and after the end. Drag the points on the stage, or the diamonds on the timeline.</p>
        <table>
          <thead><tr><th></th><th>Time (s)</th><th>X</th><th>Y</th><th></th></tr></thead>
          <tbody>
            {#each moves as m (m.f)}
              <tr class:here={m.f === app.frame}>
                <td><button class="lbl" class:start={m.label === "Start"} onclick={() => (app.frame = m.f)} title="Go to this point">{m.label}</button></td>
                <td><input type="number" min="0" step="0.1" value={secs(m.f).toFixed(2)} onchange={(e) => retime(m.f, +(e.target as HTMLInputElement).value)} aria-label="{m.label} time" /></td>
                <td><input type="number" step="1" value={m.x} onchange={(e) => setXY(m.f, +(e.target as HTMLInputElement).value, m.y)} aria-label="{m.label} x" /></td>
                <td><input type="number" step="1" value={m.y} onchange={(e) => setXY(m.f, m.x, +(e.target as HTMLInputElement).value)} aria-label="{m.label} y" /></td>
                <td class="acts">
                  <button title="Move this point to the playhead time ({fmtTime(app.frame)})" onclick={() => retime(m.f, secs(app.frame))}>⤓</button>
                  {#if moves.length > 2}<button title="Remove this point" onclick={() => removeMove(m.f)}>✕</button>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        {#if !moves.some((m) => m.f === app.frame)}
          <button class="link" onclick={addMoveHere}>＋ Add a point at {fmtTime(app.frame)}</button>
        {/if}
      </section>
    {/if}

    {#if kind.type === "character"}
      {@const ch = kind as CharacterKind}
      <section>
        <h3>Action</h3>
        <div class="chips">
          {#each app.catalog?.actions ?? [] as a}
            <button class:on={ch.action === a.id} onclick={() => editKind<CharacterKind>((k) => (k.action = a.id))}>{a.name}</button>
          {/each}
          {#each Object.values(app.project?.actions ?? {}) as a (a.id)}
            <button class="made" class:on={ch.action === a.id} title="Made from poses in this project" onclick={() => editKind<CharacterKind>((k) => (k.action = a.id))}>★ {a.name}</button>
          {/each}
        </div>
        {@render slider("Speed", ch.speed, 0.25, 2.5, 0.05, "speed", (v) => editKind<CharacterKind>((k) => (k.speed = v), "speed"), (v) => `${v.toFixed(2)}×`)}
        <label class="check"><input type="checkbox" checked={ch.autoWalk} onchange={() => editKind<CharacterKind>((k) => (k.autoWalk = !k.autoWalk))} /> Walk while moving</label>
        <label class="check"><input type="checkbox" checked={ch.autoTalk} onchange={() => editKind<CharacterKind>((k) => (k.autoTalk = !k.autoTalk))} /> Gesture while talking</label>
      </section>
      <section>
        <h3>Facing</h3>
        <div class="compass" role="radiogroup" aria-label="Which way the character faces">
          {#each VIEWS as v}
            <button
              class:on={(ch.view ?? "front") === v.id}
              style:grid-area={v.area}
              title={v.label}
              aria-label={v.label}
              onclick={() => editKind<CharacterKind>((k) => (k.view = v.id))}
            >{v.icon}</button>
          {/each}
          <span class="me" style:grid-area="c">🧍</span>
        </div>
        <label class="check"><input type="checkbox" checked={ch.autoTurn ?? true} onchange={() => editKind<CharacterKind>((k) => (k.autoTurn = !(k.autoTurn ?? true)))} /> Face the way it walks</label>
        <p class="hint small">With Auto-key on, changing the facing at a later time turns the character there.</p>
      </section>
      <PosePanel {item} />
      <section>
        <h3>Expression</h3>
        <div class="expr">
          {#each EXPRESSIONS as e}
            <button class:on={ch.expression === e.id} title={e.label} onclick={() => editKind<CharacterKind>((k) => (k.expression = e.id))}>{e.icon}</button>
          {/each}
        </div>
      </section>
      <VoicePanel {item} />
      <section class="lipsync">
        <h3>Lip sync</h3>
        <div class="field"><span>Mouth</span>
          {@render seg(ch.lipSync ?? "shapes", [{ id: "shapes", label: "Shapes" }, { id: "simple", label: "Open / close" }, { id: "off", label: "Off" }] as { id: LipSync; label: string }[], (v) => editKind<CharacterKind>((k) => (k.lipSync = v)))}
        </div>
        {#if ch.look.style !== "stick"}
          <div class="field"><span>Mouth set</span>
            {@render seg(ch.look.mouth ?? "classic", [{ id: "classic", label: "Classic" }, { id: "lips", label: "Lips" }] as { id: MouthSet; label: string }[], (v) => editKind<CharacterKind>((k) => (k.look.mouth = v)))}
          </div>
        {/if}
        <h4>Shape at {fmtTime(app.frame)}</h4>
        <div class="chips visemes">
          {#each VISEMES as v}
            <button class:on={mouthHere === v.id} title="{v.hint}. Click: this frame only. Shift-click: hold until the next mouth key." onclick={(e) => (e.shiftKey && v.id !== "auto" ? holdMouth(v.id as Viseme) : keyMouth(v.id))}>{v.label}</button>
          {/each}
        </div>
        <p class="hint small">
          Shapes follow the words of each line automatically. Pick a shape to fix this frame by hand (shift-click holds it).
          {#if mouthKeys}<button class="link" onclick={() => scene && item && run({ op: "deleteKeys", scene: scene.id, item: item.id, path: "mouth" })}>Clear {mouthKeys} mouth key{mouthKeys === 1 ? "" : "s"}</button>{/if}
        </p>
      </section>
      <section>
        <h3>Appearance</h3>
        <button class="creator-btn" onclick={() => (app.creator = { itemId: item.id })}>✎ Open character creator</button>
        <div class="field"><span>Style</span>
          {@render seg(ch.look.style ?? "cartoon", [{ id: "cartoon", label: "Cartoon" }, { id: "stick", label: "Stick figure" }], (v) => editKind<CharacterKind>((k) => {
            k.look.style = v;
            // Switching to stick: ink lines and a white head unless already customised.
            if (v === "stick") Object.assign(k.look, { skin: "#ffffff", top: "#1f2233", bottom: "#1f2233", shoes: "#1f2233" });
          }))}
        </div>
        {#if ch.look.style === "stick"}
          <label class="check"><input type="checkbox" checked={!!ch.look.dress} onchange={() => editKind<CharacterKind>((k) => (k.look.dress = !k.look.dress))} /> Dress</label>
          {#if ch.look.dress}
            <div class="field"><span>Dress colour</span>{@render swatches(ch.look.accent, PALETTE.slice(2, 12), (c) => editKind<CharacterKind>((k) => (k.look.accent = c)))}</div>
          {/if}
        {/if}
        <div class="field"><span>{ch.look.style === "stick" ? "Head" : "Skin"}</span>{@render swatches(ch.look.skin, SKIN_TONES, (c) => editKind<CharacterKind>((k) => (k.look.skin = c)))}</div>
        <div class="field"><span>Hair</span>
          {@render seg(ch.look.hairStyle, [
            { id: "short", label: "Short" }, { id: "long", label: "Long" }, { id: "spiky", label: "Spiky" },
            { id: "bun", label: "Bun" }, { id: "curly", label: "Curly" }, { id: "bald", label: "None" },
          ], (v) => editKind<CharacterKind>((k) => (k.look.hairStyle = v)))}
        </div>
        <div class="field"><span>Hair colour</span>{@render swatches(ch.look.hair, ["#1b1311", "#3b2a20", "#5a3825", "#8a5a3c", "#d99a3e", "#f2d06b", "#b23a48", "#9aa3b5"], (c) => editKind<CharacterKind>((k) => (k.look.hair = c)))}</div>
        <div class="field"><span>{ch.look.style === "stick" ? "Lines" : "Top"}</span>{@render swatches(ch.look.top, ch.look.style === "stick" ? PALETTE.slice(0, 1).concat(PALETTE.slice(2, 11)) : PALETTE.slice(2, 12), (c) => editKind<CharacterKind>((k) => (k.look.top = c)))}</div>
        {#if ch.look.style !== "stick"}<div class="field"><span>Bottom</span>{@render swatches(ch.look.bottom, ["#2f3a56", "#454d72", "#1f2233", "#264653", "#6b4630", "#9aa3b5", "#3a405a"], (c) => editKind<CharacterKind>((k) => (k.look.bottom = c)))}</div>{/if}
        <div class="field"><span>Shoes</span>{@render swatches(ch.look.shoes, ["#2a221e", "#1f2233", "#ffffff", "#e84a5f", "#8a5a3c"], (c) => editKind<CharacterKind>((k) => (k.look.shoes = c)))}</div>
        <div class="field"><span>Build</span>
          {@render seg(ch.look.build, [{ id: "slim", label: "Slim" }, { id: "regular", label: "Regular" }, { id: "broad", label: "Broad" }], (v) => editKind<CharacterKind>((k) => (k.look.build = v)))}
        </div>
        <div class="field"><span>Accessory</span>
          {@render seg(ch.look.accessory, [{ id: "none", label: "None" }, { id: "glasses", label: "Glasses" }, { id: "tie", label: "Tie" }, { id: "cap", label: "Cap" }, { id: "bow", label: "Bow" }], (v) => editKind<CharacterKind>((k) => (k.look.accessory = v)))}
        </div>
        {#if ch.look.accessory !== "none" && ch.look.accessory !== "glasses"}
          <div class="field"><span>Accessory colour</span>{@render swatches(ch.look.accent, PALETTE.slice(2, 12), (c) => editKind<CharacterKind>((k) => (k.look.accent = c)))}</div>
        {/if}
      </section>
    {:else if kind.type === "prop"}
      {@const pk = kind as PropKind}
      <section>
        <h3>Colours</h3>
        {#each propEntry?.colors ?? [] as def, i}
          <div class="field"><span>Colour {i + 1}</span>
            {@render swatches(pk.colors[i] ?? def, [def, ...PALETTE.slice(0, 9)], (c) => editKind<PropKind>((k) => {
              while (k.colors.length <= i) k.colors.push(propEntry!.colors[k.colors.length]);
              k.colors[i] = c;
            }))}
          </div>
        {/each}
        {#if pk.colors.length}<button class="link" onclick={() => editKind<PropKind>((k) => (k.colors = []))}>Reset colours</button>{/if}
      </section>
    {:else if kind.type === "text"}
      {@const tk = kind as TextKind}
        {@const legacyTw = (tk.reveal ?? "none") === "none" && tk.typewriter > 0}
        {@const byWord = tk.reveal === "popWords" || tk.reveal === "riseWords"}
      <section>
        <h3>Text</h3>
        <textarea id="text-input" rows="3" value={tk.text} oninput={(e) => editKind<TextKind>((k) => (k.text = (e.target as HTMLTextAreaElement).value), "text")} onchange={endGesture}></textarea>
        <div class="field"><span>Style</span>
          {@render seg(tk.style, [{ id: "bubble", label: "Speech" }, { id: "thought", label: "Thought" }, { id: "caption", label: "Caption" }, { id: "title", label: "Title" }, { id: "plain", label: "Plain" }], (v) => editKind<TextKind>((k) => (k.style = v)))}
        </div>
        {#if tk.style === "bubble" || tk.style === "thought"}
          <div class="field"><span>Tail</span>
            {@render seg(tk.tail, [{ id: "bottomLeft", label: "↙ Left" }, { id: "bottomRight", label: "↘ Right" }, { id: "none", label: "None" }], (v) => editKind<TextKind>((k) => (k.tail = v)))}
          </div>
        {/if}
        {@render slider("Size", tk.size, 10, 140, 1, "size", (v) => editKind<TextKind>((k) => (k.size = v), "size"))}
        {@render slider("Wrap width", tk.width, 80, 1200, 10, "width", (v) => editKind<TextKind>((k) => (k.width = v), "width"))}
        <div class="field"><span>Font</span>
          {@render seg(tk.font ?? "sans", FONTS, (v) => editKind<TextKind>((k) => (k.font = v)))}
        </div>
        <label class="check"><input type="checkbox" checked={!!tk.uppercase} onchange={() => editKind<TextKind>((k) => (k.uppercase = !k.uppercase))} /> ALL CAPS</label>
        <div class="field"><span>Text colour</span>{@render swatches(tk.color, PALETTE, (c) => editKind<TextKind>((k) => (k.color = c)))}</div>
        {@render slider("Outline", tk.strokeWidth ?? 0, 0, 16, 0.5, "sw", (v) => editKind<TextKind>((k) => { k.strokeWidth = v; if (!k.stroke) k.stroke = "#14161f"; }, "sw"), (v) => (v ? `${v}px` : "none"))}
        {#if (tk.strokeWidth ?? 0) > 0}
          <div class="field"><span>Outline colour</span>{@render swatches(tk.stroke ?? "#000000", ["#14161f", "#ffffff", ...PALETTE.slice(2, 10)], (c) => editKind<TextKind>((k) => (k.stroke = c)))}</div>
        {/if}
        {#if tk.style !== "title" && tk.style !== "plain"}
          <div class="field"><span>Fill</span>{@render swatches(tk.fill, ["#ffffff", "#141726cc", "#fff4c2", "#e8f1ff", ...PALETTE.slice(2, 8)], (c) => editKind<TextKind>((k) => (k.fill = c)))}</div>
        {/if}
      </section>
      <section>
        <h3>Text animation</h3>
        <div class="row2">
          <label class="field"><span>Letters come in</span>
            <select value={legacyTw ? "typewriter" : (tk.reveal ?? "none")} onchange={(e) => editKind<TextKind>((k) => {
              k.reveal = (e.target as HTMLSelectElement).value as Reveal;
              if (k.typewriter > 0) { k.revealSpeed = k.typewriter; k.typewriter = 0; }
              if (k.reveal === "popWords" || k.reveal === "riseWords") k.revealSpeed = Math.min(k.revealSpeed ?? 4, 8);
              else if ((k.revealSpeed ?? 20) < 8) k.revealSpeed = 18;
            })}>
              {#each REVEALS as r}<option value={r.id}>{r.label}</option>{/each}
            </select>
          </label>
          <label class="field"><span>Letters leave</span>
            <select value={tk.revealOut ?? "none"} onchange={(e) => editKind<TextKind>((k) => (k.revealOut = (e.target as HTMLSelectElement).value as Reveal))}>
              {#each REVEALS as r}<option value={r.id}>{r.label}</option>{/each}
            </select>
          </label>
        </div>
        {#if (tk.reveal ?? "none") !== "none" || (tk.revealOut ?? "none") !== "none" || legacyTw}
          {@render slider("Speed", legacyTw ? tk.typewriter : (tk.revealSpeed ?? 20), 1, byWord ? 12 : 60, 1, "rs", (v) => editKind<TextKind>((k) => { if (k.typewriter > 0) { k.reveal = "typewriter"; k.typewriter = 0; } k.revealSpeed = v; }, "rs"), (v) => `${v} ${byWord ? "words" : "letters"}/s`)}
        {/if}
        {#if (tk.revealOut ?? "none") !== "none" && item.end === null}
          <p class="hint small">Letters leave before the text's end time: turn off "Stay until the scene ends" under Timing.</p>
        {/if}
        <div class="field"><span>Loop</span>
          {@render seg(tk.emphasis ?? "none", EMPHASES, (v) => editKind<TextKind>((k) => (k.emphasis = v)))}
        </div>
        <label class="check" title="Stays in the same place in the frame while the camera pans or zooms, and draws on top of everything">
          <input type="checkbox" checked={!!tk.pinned} onchange={() => edit((it) => {
            const k = it.kind as TextKind;
            k.pinned = !k.pinned;
            // Keep it where it is on screen (for a still camera).
            const c = scene!.camera.from;
            if (k.pinned) { it.x = Math.round((it.x - c.x) * c.zoom + 640); it.y = Math.round((it.y - c.y) * c.zoom + 360); }
            else { it.x = Math.round((it.x - 640) / c.zoom + c.x); it.y = Math.round((it.y - 360) / c.zoom + c.y); }
          })} /> Pin to screen (ignore camera)
        </label>
        {#if tk.pinned}
          <div class="field"><span>Place in frame</span>
            <div class="seg">
              {#each PIN_SPOTS as s}
                <button onclick={() => edit((it) => { it.x = 640; it.y = s.y; })}>{s.label}</button>
              {/each}
            </div>
          </div>
        {/if}
        <p class="hint small">Position, size, rotation, opacity and colours can be keyframed: turn on Auto-key, move the playhead and change them.</p>
      </section>
    {/if}

    <section>
      <h3>Placement</h3>
      <div class="row2">
        <label class="field num"><span>X</span><input type="number" value={item.x} onchange={(e) => edit((it) => (it.x = +(e.target as HTMLInputElement).value))} /></label>
        <label class="field num"><span>Y</span><input type="number" value={item.y} onchange={(e) => edit((it) => (it.y = +(e.target as HTMLInputElement).value))} /></label>
      </div>
      {@render slider("Scale", item.scale, 0.1, 3, 0.01, "scale", (v) => edit((it) => (it.scale = v), "scale"), (v) => `${Math.round(v * 100)}%`)}
      {@render slider("Rotation", item.rotation, -180, 180, 1, "rot", (v) => edit((it) => (it.rotation = v), "rot"), (v) => `${v}°`)}
      {@render slider("Opacity", item.opacity, 0, 1, 0.01, "op", (v) => edit((it) => (it.opacity = v), "op"), (v) => `${Math.round(v * 100)}%`)}
    </section>

    <section>
      <h3>Timing</h3>
      <div class="row2">
        <label class="field num"><span>Appears (s)</span>
          <input type="number" min="0" step="0.1" value={secs(item.start).toFixed(1)} onchange={(e) => edit((it) => (it.start = Math.max(0, frames(+(e.target as HTMLInputElement).value))))} />
        </label>
        <label class="field num"><span>Leaves (s)</span>
          <input type="number" min="0" step="0.1" disabled={item.end === null} value={item.end === null ? "" : secs(item.end).toFixed(1)} placeholder="end"
            onchange={(e) => edit((it) => (it.end = Math.max(it.start + 1, frames(+(e.target as HTMLInputElement).value))))} />
        </label>
      </div>
      <label class="check"><input type="checkbox" checked={item.end === null} onchange={() => edit((it) => (it.end = it.end === null ? Math.min(scene!.duration, it.start + frames(3)) : null))} /> Stay until the scene ends</label>
      <div class="row2">
        <label class="field"><span>Enter</span>
          <select value={item.enter} onchange={(e) => edit((it) => (it.enter = (e.target as HTMLSelectElement).value as Effect))}>
            {#each effects as ef}<option value={ef.id}>{ef.label}</option>{/each}
          </select>
        </label>
        <label class="field"><span>Exit</span>
          <select value={item.exit} onchange={(e) => edit((it) => (it.exit = (e.target as HTMLSelectElement).value as Effect))}>
            {#each effects as ef}<option value={ef.id}>{ef.label}</option>{/each}
          </select>
        </label>
      </div>
    </section>

    <section>
      <h3>Movement</h3>
      {#if item.motion}
        <p class="hint">Drag the yellow arrow on the stage to set where it goes.</p>
        <div class="row2">
          <label class="field num"><span>From (s)</span>
            <input type="number" min="0" step="0.1" value={secs(item.motion.start).toFixed(1)} onchange={(e) => edit((it) => (it.motion!.start = Math.max(0, frames(+(e.target as HTMLInputElement).value))))} />
          </label>
          <label class="field num"><span>To (s)</span>
            <input type="number" min="0" step="0.1" value={secs(item.motion.end).toFixed(1)} onchange={(e) => edit((it) => (it.motion!.end = Math.max(it.motion!.start + 1, frames(+(e.target as HTMLInputElement).value))))} />
          </label>
        </div>
        <div class="field"><span>Easing</span>
          {@render seg(item.motion.ease, [{ id: "linear", label: "Steady" }, { id: "inOut", label: "Smooth" }, { id: "out", label: "Slow end" }, { id: "back", label: "Bouncy" }], (v) => edit((it) => (it.motion!.ease = v)))}
        </div>
        <button class="link" onclick={() => edit((it) => (it.motion = null))}>Remove movement</button>
      {:else}
        <button class="ghost wide" onclick={() => edit((it) => (it.motion = { x: it.x + 300, y: it.y, start: app.frame, end: app.frame + frames(2), ease: "inOut" }))}>➜ Add movement</button>
      {/if}
    </section>
  {:else if scene && project}
    <header><input class="name" value={scene.name} onchange={(e) => editScene((s) => (s.name = (e.target as HTMLInputElement).value))} /><span class="badge scene">scene</span></header>
    <section>
      <h3>Scene</h3>
      {@render slider("Length", scene.duration, frames(1), frames(60), 1, "dur", (v) => editScene((s) => (s.duration = v), "dur"), (v) => `${secs(v).toFixed(1)}s`)}
      {#if bgEntry?.colors.length}
        {#each bgEntry.colors as def, i}
          <div class="field"><span>Background colour {i + 1}</span>
            {@render swatches(scene.background.colors[i] ?? def, [def, ...PALETTE.slice(0, 9)], (c) => editScene((s) => {
              while (s.background.colors.length <= i) s.background.colors.push(bgEntry!.colors[s.background.colors.length]);
              s.background.colors[i] = c;
            }))}
          </div>
        {/each}
      {/if}
    </section>
    <section>
      <h3>Transition in</h3>
      {#if app.sceneIndex === 0}<p class="hint">The first scene starts the movie, so it has no transition.</p>{/if}
      {@render seg(scene.transition.kind, transitions, (v) => editScene((s) => (s.transition.kind = v)))}
      {#if scene.transition.kind !== "cut"}
        {@render slider("Duration", scene.transition.duration, 3, 60, 1, "tdur", (v) => editScene((s) => (s.transition.duration = v), "tdur"), (v) => `${secs(v).toFixed(2)}s`)}
      {/if}
    </section>
    <section>
      <h3>Camera</h3>
      {@render seg(scene.camera.moving ? "move" : "still", [{ id: "still", label: "Still shot" }, { id: "move", label: "Pan / zoom" }], (v) => editScene((s) => {
        s.camera.moving = v === "move";
        if (s.camera.moving && s.camera.to.zoom === s.camera.from.zoom && s.camera.to.x === s.camera.from.x) s.camera.to = { ...s.camera.from, zoom: s.camera.from.zoom * 1.3 };
      }))}
      <h4>{scene.camera.moving ? "Start" : "View"}</h4>
      {@render slider("Zoom", scene.camera.from.zoom, 0.5, 3, 0.01, "cz0", (v) => editScene((s) => (s.camera.from.zoom = v), "cz0"), (v) => `${v.toFixed(2)}×`)}
      {@render slider("Pan X", scene.camera.from.x, 0, 1280, 1, "cx0", (v) => editScene((s) => (s.camera.from.x = v), "cx0"))}
      {@render slider("Pan Y", scene.camera.from.y, 0, 720, 1, "cy0", (v) => editScene((s) => (s.camera.from.y = v), "cy0"))}
      {#if scene.camera.moving}
        <h4>End</h4>
        {@render slider("Zoom", scene.camera.to.zoom, 0.5, 3, 0.01, "cz1", (v) => editScene((s) => (s.camera.to.zoom = v), "cz1"), (v) => `${v.toFixed(2)}×`)}
        {@render slider("Pan X", scene.camera.to.x, 0, 1280, 1, "cx1", (v) => editScene((s) => (s.camera.to.x = v), "cx1"))}
        {@render slider("Pan Y", scene.camera.to.y, 0, 720, 1, "cy1", (v) => editScene((s) => (s.camera.to.y = v), "cy1"))}
      {/if}
    </section>
    {#if captions}
      <section class="captions">
        <h3>Auto subtitles</h3>
        <label class="check"><input type="checkbox" checked={captions.enabled} onchange={toggleCaptions} /> Show subtitles from the voice lines</label>
        {#if captions.enabled}
          <div class="field"><span>Look</span>
            <div class="seg">
              {#each CAPTION_STYLES as s}
                <button class:on={captions.style === s.id} title={s.hint} onclick={() => editCaptions((c) => (c.style = s.id))}>{s.label}</button>
              {/each}
            </div>
          </div>
          <div class="field"><span>Font</span>{@render seg(captions.font, FONTS, (v) => editCaptions((c) => (c.font = v)))}</div>
          {@render slider("Size", captions.size, 16, 110, 1, "csz", (v) => editCaptions((c) => (c.size = v), "csz"))}
          {@render slider("Words at once", captions.words, 1, 8, 1, "cw", (v) => editCaptions((c) => (c.words = v), "cw"))}
          <div class="field"><span>Position</span>
            {@render seg(captions.y < 0.34 ? "top" : captions.y < 0.62 ? "middle" : "bottom", [{ id: "top", label: "Top" }, { id: "middle", label: "Middle" }, { id: "bottom", label: "Bottom" }], (v) => editCaptions((c) => (c.y = v === "top" ? 0.2 : v === "middle" ? 0.5 : project.height > project.width ? 0.7 : 0.84)))}
          </div>
          {@render slider("Height", Math.round(captions.y * 100), 5, 95, 1, "cy", (v) => editCaptions((c) => (c.y = v / 100), "cy"), (v) => `${v}%`)}
          <div class="field"><span>Text</span>{@render swatches(captions.color, ["#ffffff", "#14161f", "#ffd23f", ...PALETTE.slice(2, 8)], (v) => editCaptions((c) => (c.color = v)))}</div>
          {#if captions.style !== "clean"}
            <div class="field"><span>Highlight</span>{@render swatches(captions.highlight, ["#ffd23f", "#3ddc84", "#ff4d6d", "#4f7cff", "#7c5cff", "#ff9f1c", "#22d3ee", "#ffffff"], (v) => editCaptions((c) => (c.highlight = v)))}</div>
          {/if}
          {#if captions.style === "boxed"}
            <div class="field"><span>Panel</span>{@render swatches(captions.boxColor, ["#10121cc8", "#000000", "#ffffffe6", "#4f7cffd9", "#e84a5fd9"], (v) => editCaptions((c) => (c.boxColor = v)))}</div>
          {:else}
            {@render slider("Outline", captions.strokeWidth, 0, 12, 0.5, "csw", (v) => editCaptions((c) => (c.strokeWidth = v), "csw"), (v) => (v ? `${v}px` : "none"))}
            {#if captions.strokeWidth > 0}
              <div class="field"><span>Outline colour</span>{@render swatches(captions.stroke, ["#101014", "#ffffff", "#4f7cff", "#e84a5f"], (v) => editCaptions((c) => (c.stroke = v)))}</div>
            {/if}
          {/if}
          <label class="check"><input type="checkbox" checked={captions.uppercase} onchange={() => editCaptions((c) => (c.uppercase = !c.uppercase))} /> ALL CAPS</label>
          <label class="check"><input type="checkbox" checked={captions.pop} onchange={() => editCaptions((c) => (c.pop = !c.pop))} /> Pop each group in</label>
          <button class="ghost wide" onclick={saveSubtitles} title="A subtitle file to upload next to the video">⬇ Save subtitles (.srt)</button>
          <p class="hint small">Subtitles cover the whole movie and follow each voice line. Fix a line's words under Sounds below.</p>
        {/if}
      </section>
    {/if}
    <section>
      <h3>Sounds in this scene</h3>
      {#each scene.audio as c (c.id)}
        <div class="clip-card">
          <div class="clip"><span>♪ {project.media[c.media]?.name ?? "missing"}</span>
            <button class="x" title="Remove" onclick={() => run({ op: "removeAudio", scene: scene.id, id: c.id })}>✕</button></div>
          <div class="row2">
            <label class="field num"><span>Starts (s)</span><input type="number" step="0.1" min="0" value={secs(c.start).toFixed(1)} onchange={(e) => editClip(c, (n) => (n.start = Math.max(0, frames(+(e.target as HTMLInputElement).value))))} /></label>
            <label class="field"><span>Spoken by</span>
              <select value={c.speaker ?? ""} onchange={(e) => editClip(c, (n) => (n.speaker = (e.target as HTMLSelectElement).value || null))}>
                <option value="">Nobody (music / effect)</option>
                {#each characters as ch}<option value={ch.id}>{ch.name}</option>{/each}
              </select>
            </label>
          </div>
          {@render slider("Volume", c.volume, 0, 2, 0.01, "vol", (v) => editClip(c, (n) => (n.volume = v), "vol"), (v) => `${Math.round(v * 100)}%`)}
          {#if captions?.enabled}
            <label class="field"><span>Subtitle{isVoiceLine(c) ? "" : " (type the words)"}</span>
              <textarea rows="2" value={c.caption ?? lineText(c)} placeholder={isVoiceLine(c) ? "" : "No subtitle"}
                onchange={(e) => editClip(c, (n) => {
                  const v = (e.target as HTMLTextAreaElement).value;
                  n.caption = isVoiceLine(c) && v.trim() === lineText(c).trim() ? null : v;
                })}></textarea>
            </label>
          {/if}
        </div>
      {:else}
        <p class="hint">No sounds yet. Use the Sound tab to import voice or music.</p>
      {/each}
    </section>
    <section>
      <h3>Project</h3>
      <label class="field"><span>Title</span><input value={project.name} onchange={(e) => run({ op: "setMeta", name: (e.target as HTMLInputElement).value })} /></label>
      <div class="row2">
        <label class="field"><span>Size</span>
          <select value={`${project.width}x${project.height}`} onchange={(e) => { const [w, h] = (e.target as HTMLSelectElement).value.split("x").map(Number); run({ op: "setMeta", width: w, height: h }); }}>
            {#each sizes as s}<option value={`${s.w}x${s.h}`}>{s.label}</option>{/each}
          </select>
        </label>
        <label class="field"><span>Frame rate</span>
          <select value={String(project.fps)} onchange={(e) => run({ op: "setMeta", fps: +(e.target as HTMLSelectElement).value })}>
            {#each [24, 25, 30, 60] as f}<option value={String(f)}>{f} fps</option>{/each}
          </select>
        </label>
      </div>
      <p class="hint">Tip: changing the frame rate keeps timings in frames, so set it before you start.</p>
    </section>
  {/if}
</aside>

<style>
  .inspector {
    width: 300px;
    flex-shrink: 0;
    border-left: 1px solid var(--line);
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 14px 24px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 0 10px;
    position: sticky;
    top: 0;
    background: var(--bg-1);
    z-index: 1;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 15px;
    font-weight: 600;
    background: transparent;
    border-color: transparent;
    padding-left: 4px;
  }
  .name:hover,
  .name:focus {
    border-color: var(--line);
    background: var(--bg-2);
  }
  .badge {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 3px 7px;
    border-radius: 6px;
    background: var(--bg-3);
    color: var(--muted);
  }
  .badge.character { color: #8fb0ff; }
  .badge.prop { color: #7fd8a4; }
  .badge.text { color: #ffcf5a; }
  .actions {
    display: flex;
    gap: 4px;
    margin-bottom: 6px;
  }
  .actions button {
    padding: 6px 8px;
    font-size: 12px;
  }
  .actions button:first-child {
    flex: 1;
  }
  .actions .on {
    border-color: var(--accent);
    color: var(--text);
  }
  .keys {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 14px;
    margin-top: 8px;
  }
  .keys button {
    padding: 4px 7px;
    font-size: 11px;
    white-space: nowrap;
  }
  .keys button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .keys .auto {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-right: auto;
  }
  .keys .auto .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted);
  }
  .keys .auto.on {
    color: #ff6b6b;
    border-color: #ff6b6b66;
  }
  .keys .auto.on .dot {
    background: #ff4d4d;
    box-shadow: 0 0 6px #ff4d4d;
  }
  .diamond {
    font-size: 13px;
    color: var(--muted);
    padding: 0 2px;
  }
  .diamond.on {
    color: #ffcf5a;
  }
  .moves table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  .moves th {
    font-weight: 500;
    color: var(--muted);
    text-align: left;
    padding: 0 3px 4px;
  }
  .moves td {
    padding: 2px 3px;
  }
  .moves input {
    width: 100%;
    min-width: 0;
    padding: 4px 5px;
  }
  .moves tr.here .lbl {
    color: #ffcf5a;
  }
  .moves .lbl {
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    white-space: nowrap;
  }
  .moves .lbl.start::before {
    content: "● ";
    color: #3fae6a;
  }
  .moves .acts {
    white-space: nowrap;
  }
  .moves .acts button {
    width: 22px;
    height: 22px;
    padding: 0;
    font-size: 11px;
  }
  .moves .link {
    background: none;
    border: none;
    color: var(--accent-2);
    font-size: 12px;
    padding: 6px 0 0;
  }
  .keyhint {
    margin: 6px 14px 0;
    font-size: 11px;
    line-height: 1.4;
    color: var(--muted);
  }
  .creator-btn {
    width: 100%;
    margin-bottom: 10px;
    padding: 8px;
    font-weight: 600;
    border-color: var(--accent);
    color: var(--accent-2);
  }
  section {
    border-top: 1px solid var(--line);
    padding: 12px 0 6px;
  }
  h3 {
    margin: 0 0 10px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
  }
  h4 {
    margin: 10px 0 4px;
    font-size: 12px;
    color: var(--muted);
    font-weight: 500;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-bottom: 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .field.slider {
    display: grid;
    grid-template-columns: 74px 1fr 52px;
    align-items: center;
    gap: 8px;
  }
  .field.slider output {
    text-align: right;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .row2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .num input {
    width: 100%;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 12.5px;
    margin: 4px 0 8px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 10px;
  }
  .chips button {
    padding: 5px 10px;
    font-size: 12px;
    border-radius: 999px;
  }
  .chips .on,
  .seg .on,
  .expr .on {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  .expr {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 4px;
  }
  .expr button {
    font-size: 19px;
    padding: 4px 0;
  }
  .seg {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    background: var(--bg-2);
    border-radius: 9px;
    padding: 3px;
    border: 1px solid var(--line);
    margin-bottom: 6px;
  }
  .seg button {
    flex: 1;
    padding: 5px 6px;
    font-size: 11.5px;
    border: 1px solid transparent;
    background: transparent;
    white-space: nowrap;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .sw {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.12);
    padding: 0;
  }
  .sw.on {
    border-color: white;
    box-shadow: 0 0 0 2px var(--accent);
  }
  .sw.custom {
    position: relative;
    overflow: hidden;
    background: conic-gradient(#f55, #fd5, #5f8, #5cf, #85f, #f5c, #f55);
    cursor: pointer;
  }
  .sw.custom input {
    opacity: 0;
    position: absolute;
    inset: 0;
    cursor: pointer;
  }
  textarea {
    width: 100%;
    resize: vertical;
    margin-bottom: 10px;
  }
  .hint {
    color: var(--muted);
    font-size: 12px;
    line-height: 1.45;
    margin: 0 0 8px;
  }
  .clip {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 12.5px;
    margin-bottom: 6px;
  }
  .clip-card {
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 8px 10px 2px;
    margin-bottom: 8px;
  }
  .x {
    padding: 2px 7px;
    font-size: 11px;
  }
  .dim {
    color: var(--muted);
  }
  .wide {
    width: 100%;
  }
  .link {
    background: none;
    border: none;
    color: var(--accent-2);
    padding: 0;
    font-size: 12px;
    margin-bottom: 8px;
  }
  .danger:hover {
    border-color: #e84a5f;
    color: #ff8a98;
  }
  .visemes button {
    padding: 4px 8px;
    font-size: 11px;
  }
  .chips .made {
    border-style: dashed;
  }
  .compass {
    display: grid;
    grid-template-areas: "nw n ne" "w c e" "sw s se";
    grid-template-columns: repeat(3, 34px);
    grid-template-rows: repeat(3, 30px);
    gap: 3px;
    justify-content: center;
    margin: 4px 0 8px;
  }
  .compass button {
    padding: 0;
    font-size: 14px;
  }
  .compass button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .compass .me {
    display: grid;
    place-items: center;
    font-size: 16px;
    opacity: 0.6;
  }
  .hint.small {
    font-size: 11px;
    color: var(--muted);
    margin: 4px 0 0;
  }
</style>
