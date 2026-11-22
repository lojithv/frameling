<script lang="ts">
  import { app, currentScene, selectedItem, updateItem, updateScene, endGesture, run, secs, frames } from "../lib/store.svelte";
  import { EXPRESSIONS, PALETTE, SKIN_TONES, uid } from "../lib/defaults";
  import VoicePanel from "./VoicePanel.svelte";
  import type { AudioClip, CharacterKind, Effect, Item, PropKind, Scene, TextKind, TransitionKind } from "../lib/types";

  const scene = $derived(currentScene());
  const item = $derived(selectedItem());
  const project = $derived(app.project);

  const snap = <T,>(v: T): T => structuredClone($state.snapshot(v)) as T;

  /** Edit the selected item. `key` merges a slider drag into one undo step. */
  function edit(fn: (it: Item) => void, key?: string) {
    if (!item) return;
    const it = snap(item);
    fn(it);
    updateItem(it, key);
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
  ];
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

    {#if kind.type === "character"}
      {@const ch = kind as CharacterKind}
      <section>
        <h3>Action</h3>
        <div class="chips">
          {#each app.catalog?.actions ?? [] as a}
            <button class:on={ch.action === a.id} onclick={() => editKind<CharacterKind>((k) => (k.action = a.id))}>{a.name}</button>
          {/each}
        </div>
        {@render slider("Speed", ch.speed, 0.25, 2.5, 0.05, "speed", (v) => editKind<CharacterKind>((k) => (k.speed = v), "speed"), (v) => `${v.toFixed(2)}×`)}
        <label class="check"><input type="checkbox" checked={ch.autoWalk} onchange={() => editKind<CharacterKind>((k) => (k.autoWalk = !k.autoWalk))} /> Walk while moving</label>
        <label class="check"><input type="checkbox" checked={ch.autoTalk} onchange={() => editKind<CharacterKind>((k) => (k.autoTalk = !k.autoTalk))} /> Gesture while talking</label>
      </section>
      <section>
        <h3>Expression</h3>
        <div class="expr">
          {#each EXPRESSIONS as e}
            <button class:on={ch.expression === e.id} title={e.label} onclick={() => editKind<CharacterKind>((k) => (k.expression = e.id))}>{e.icon}</button>
          {/each}
        </div>
      </section>
      <VoicePanel {item} />
      <section>
        <h3>Appearance</h3>
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
        {@render slider("Typewriter", tk.typewriter, 0, 60, 1, "tw", (v) => editKind<TextKind>((k) => (k.typewriter = v), "tw"), (v) => (v ? `${v} chars/s` : "off"))}
        <div class="field"><span>Text colour</span>{@render swatches(tk.color, PALETTE, (c) => editKind<TextKind>((k) => (k.color = c)))}</div>
        {#if tk.style !== "title" && tk.style !== "plain"}
          <div class="field"><span>Fill</span>{@render swatches(tk.fill, ["#ffffff", "#141726cc", "#fff4c2", "#e8f1ff", ...PALETTE.slice(2, 8)], (c) => editKind<TextKind>((k) => (k.fill = c)))}</div>
        {/if}
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
      {@render slider("Pan X", scene.camera.from.x, 0, project.width, 1, "cx0", (v) => editScene((s) => (s.camera.from.x = v), "cx0"))}
      {@render slider("Pan Y", scene.camera.from.y, 0, project.height, 1, "cy0", (v) => editScene((s) => (s.camera.from.y = v), "cy0"))}
      {#if scene.camera.moving}
        <h4>End</h4>
        {@render slider("Zoom", scene.camera.to.zoom, 0.5, 3, 0.01, "cz1", (v) => editScene((s) => (s.camera.to.zoom = v), "cz1"), (v) => `${v.toFixed(2)}×`)}
        {@render slider("Pan X", scene.camera.to.x, 0, project.width, 1, "cx1", (v) => editScene((s) => (s.camera.to.x = v), "cx1"))}
        {@render slider("Pan Y", scene.camera.to.y, 0, project.height, 1, "cy1", (v) => editScene((s) => (s.camera.to.y = v), "cy1"))}
      {/if}
    </section>
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
</style>
