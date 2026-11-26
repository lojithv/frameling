// Keyboard editing helpers: nudge, duplicate, copy and paste. Kept apart from
// App.svelte so the rules for animated items live in one place.

import { app, currentScene, selectedItem, posedItem, editItem, updateItem, endGesture, run, toast } from "./store.svelte";
import { uid } from "./defaults";
import type { Item, Scene } from "./types";

/** Old-style movement (a motion target, no keyframes): edited as the document item. */
function legacyMotion(it: Item): boolean {
  return !!it.motion && !Object.keys(it.tracks ?? {}).length;
}

/**
 * A plain copy of an item moved by (dx, dy) everywhere it appears: its base
 * place, every position key and an old-style movement target. Moving only the
 * base would do nothing on an animated item, whose keys override it.
 */
export function shifted(item: Item, dx: number, dy: number): Item {
  const it = structuredClone($state.snapshot(item) as Item);
  it.x += dx;
  it.y += dy;
  if (it.motion) {
    it.motion.x += dx;
    it.motion.y += dy;
  }
  for (const [path, d] of [["transform.x", dx], ["transform.y", dy]] as const) {
    for (const k of it.tracks?.[path]?.keys ?? []) if (typeof k.value === "number") k.value += d;
  }
  return it;
}

/**
 * Shift every frame of an item (its span, keys and old-style movement) by
 * `df`, so it can land in a scene of another length.
 */
function retimed(it: Item, df: number): Item {
  if (!df) return it;
  it.start = Math.max(0, it.start + df);
  if (it.end != null) it.end = Math.max(it.start + 1, it.end + df);
  if (it.motion) {
    it.motion.start = Math.max(0, it.motion.start + df);
    it.motion.end = Math.max(it.motion.start, it.motion.end + df);
  }
  for (const t of Object.values(it.tracks ?? {})) {
    for (const k of t.keys) k.frame = Math.max(0, k.frame + df);
    // Keys pushed onto the same frame: keep the last one.
    const seen = new Map<number, (typeof t.keys)[number]>();
    for (const k of t.keys) seen.set(k.frame, k);
    t.keys = [...seen.values()].sort((a, b) => a.frame - b.frame);
  }
  return it;
}

// ------------------------------------------------------------------ nudge
// A burst of arrow presses is one undo step: the gesture is sealed once the
// keys have been quiet for a moment.
let nudgeKey = "";
let nudgeTimer: ReturnType<typeof setTimeout> | undefined;

/** Move the selected item by (dx, dy) at the playhead, like a drag would. */
export function nudgeSelected(dx: number, dy: number) {
  const item = selectedItem();
  if (!item || item.locked) return;
  if (!nudgeKey) nudgeKey = `nudge-${Date.now()}`;
  clearTimeout(nudgeTimer);
  nudgeTimer = setTimeout(() => {
    endGesture();
    nudgeKey = "";
  }, 700);
  if (legacyMotion(item)) {
    // Same as dragging an old-style item: the base place moves.
    const it = structuredClone($state.snapshot(item) as Item);
    it.x += dx;
    it.y += dy;
    updateItem(it, nudgeKey);
    return;
  }
  // An edit at the playhead: keys it when the item is animated (or auto-key is on).
  const it = posedItem(item, app.frame);
  it.x += dx;
  it.y += dy;
  editItem(it, nudgeKey);
}

// -------------------------------------------------------------- duplicate
/** Duplicate the selected item 40 px to the right, keeping its animation. */
export function duplicateSelected() {
  const scene = currentScene();
  const item = selectedItem();
  if (!scene || !item) return;
  const copy = shifted(item, 40, 0);
  copy.id = uid(copy.kind.type);
  // Just above the original in the stacking order.
  const index = scene.items.findIndex((i) => i.id === item.id) + 1;
  if (run({ op: "addItem", scene: scene.id, item: copy, index })) app.selected = copy.id;
}

// --------------------------------------------------------- copy and paste
// An in-app clipboard: items keep their media references, which only make
// sense inside this project, so the system clipboard is not used.
let clip: { item: Item; scene: string; pasted: number } | null = null;

export function copySelected(): boolean {
  const scene = currentScene();
  const item = selectedItem();
  if (!scene || !item) return false;
  clip = { item: structuredClone($state.snapshot(item) as Item), scene: scene.id, pasted: 0 };
  toast(`Copied ${item.name || "item"}`);
  return true;
}

export function cutSelected(): boolean {
  const scene = currentScene();
  const item = selectedItem();
  if (!scene || !item || !copySelected()) return false;
  run({ op: "removeItem", scene: scene.id, id: item.id });
  toast(`Cut ${item.name || "item"}`);
  return true;
}

export function hasClip() {
  return !!clip;
}

/**
 * Paste the copied item into the current scene. In the scene it came from it
 * lands offset (further each time), elsewhere in the same place. If it starts
 * after the scene ends, it is moved to start at the playhead.
 */
export function paste(): boolean {
  const scene = currentScene();
  if (!scene || !clip) return false;
  const same = clip.scene === scene.id;
  const n = same ? clip.pasted + 1 : 0;
  let it = shifted(clip.item, 40 * n, 40 * n);
  it.id = uid(it.kind.type);
  if (it.start >= scene.duration) it = retimed(it, app.frame - it.start);
  if (!run({ op: "addItem", scene: scene.id, item: it })) return false;
  if (same) clip.pasted = n;
  app.selected = it.id;
  toast(`Pasted ${it.name || "item"}${same ? "" : ` into ${sceneName(scene)}`}`);
  return true;
}

function sceneName(s: Scene) {
  return s.name || "this scene";
}
