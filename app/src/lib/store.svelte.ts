// App state. The Rust engine owns the document; this store holds a read-only
// copy of it for the UI plus view state (selection, playhead, panels).

import init, { Engine } from "../engine/frameling.js";
import type { Catalog, Command, Item, ItemHit, Project, Scene } from "./types";

let engine: Engine | null = null;

// Declared before `app`, whose initialiser reads them.
const RECENT_KEY = "frameling.recent";
const RECENT_MAX = 10;

export const app = $state({
  ready: false,
  project: null as Project | null,
  catalog: null as Catalog | null,
  sceneIndex: 0,
  /** Local frame within the current scene (editing playhead). */
  frame: 0,
  selected: null as string | null,
  playing: false,
  /** "scene" plays the current scene, "all" plays the whole movie with transitions. */
  playMode: "scene" as "scene" | "all",
  globalFrame: 0,
  canUndo: false,
  canRedo: false,
  dirty: false,
  /** Bumped whenever the picture may have changed. */
  rev: 0,
  hits: [] as ItemHit[],
  toast: null as null | { text: string; kind: "info" | "error" },
  filePath: null as string | null,
  /** Last known modification time of filePath (desktop), to notice agent edits. */
  fileMtime: 0,
  exporting: null as null | { done: number; total: number; label: string },
  /** Changes made at the playhead become keyframes (see editItem). */
  autoKey: readAutoKey(),
  /** Desktop: project files opened or saved lately, newest first. */
  recent: readRecent(),
});

export interface RecentProject {
  path: string;
  name: string;
  /** Last opened or saved, ms since epoch. */
  at: number;
}

function readRecent(): RecentProject[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((r) => typeof r?.path === "string") : [];
  } catch {
    return [];
  }
}
function writeRecent() {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(app.recent));
  } catch {
    /* private mode */
  }
}
/** Put a file at the top of the recent list. */
export function rememberRecent(path: string, name: string) {
  app.recent = [{ path, name: name || "Untitled", at: Date.now() }, ...app.recent.filter((r) => r.path !== path)].slice(0, RECENT_MAX);
  writeRecent();
}
export function forgetRecent(path?: string) {
  app.recent = path ? app.recent.filter((r) => r.path !== path) : [];
  writeRecent();
}

function readAutoKey(): boolean {
  try {
    return localStorage.getItem("frameling.autoKey") !== "off";
  } catch {
    return true;
  }
}
export function setAutoKey(on: boolean) {
  app.autoKey = on;
  try {
    localStorage.setItem("frameling.autoKey", on ? "on" : "off");
  } catch {
    /* private mode */
  }
}

export async function boot() {
  await init();
  engine = new Engine();
  const saved = await loadAutosave();
  let restored = false;
  if (saved) {
    try {
      engine.load(saved);
      restored = true;
    } catch {
      /* ignore broken autosave */
    }
  }
  if (!restored) engine.load_demo();
  app.catalog = JSON.parse(engine.catalog());
  refresh();
  app.ready = true;
  if (restored) toast("Restored your last session");
}

export function eng(): Engine {
  if (!engine) throw new Error("engine not ready");
  return engine;
}

/** Re-read the project from the engine after any change. */
export function refresh() {
  const e = eng();
  app.project = JSON.parse(e.project_view_json());
  app.canUndo = e.can_undo();
  app.canRedo = e.can_redo();
  app.dirty = e.is_dirty();
  const n = app.project!.scenes.length;
  if (app.sceneIndex >= n) app.sceneIndex = n - 1;
  const s = currentScene();
  if (s && app.frame >= s.duration) app.frame = Math.max(0, s.duration - 1);
  if (app.selected && !s?.items.some((i) => i.id === app.selected)) app.selected = null;
  app.rev++;
  scheduleAutosave();
}

/** Apply a command. `merge` collapses a continuous gesture into one undo step. */
export function run(cmd: Command, merge?: string): boolean {
  try {
    eng().apply(JSON.stringify(cmd), merge ?? undefined);
    refresh();
    return true;
  } catch (e) {
    toast(String((e as Error).message ?? e), "error");
    return false;
  }
}

export function endGesture() {
  eng().seal();
}

export function undo() {
  if (eng().undo()) refresh();
}
export function redo() {
  if (eng().redo()) refresh();
}

export function currentScene(): Scene | null {
  return app.project?.scenes[app.sceneIndex] ?? null;
}

export function selectedItem(): Item | null {
  const s = currentScene();
  return s?.items.find((i) => i.id === app.selected) ?? null;
}

/** An item as it looks at the playhead: keyed values baked into its fields. */
export function posedItem(item: Item, frame = app.frame): Item {
  if (!item.tracks || Object.keys(item.tracks).length === 0) return item;
  try {
    return JSON.parse(eng().posed_item(JSON.stringify($state.snapshot(item)), frame));
  } catch {
    return item;
  }
}

/** The selected item posed at the playhead (what the inspector shows). */
export function selectedPosed(): Item | null {
  const it = selectedItem();
  return it ? posedItem(it, app.frame) : null;
}

/**
 * Store an edit of a posed item made at the playhead. With auto-key on, a
 * changed property becomes a keyframe at the playhead (its first change past
 * the item's first frame also keys the old value there); properties that are
 * already animated are always keyed. Other fields are stored as they are.
 */
export function editItem(item: Item, merge?: string) {
  const s = currentScene();
  if (s) run({ op: "autoKey", scene: s.id, item, frame: app.frame, auto: app.autoKey }, merge);
}

/** Every frame with a key on the item, sorted. */
export function keyFrames(item: Item): number[] {
  const set = new Set<number>();
  for (const t of Object.values(item.tracks ?? {})) for (const k of t.keys) set.add(k.frame);
  return [...set].sort((a, b) => a - b);
}

export function updateItem(item: Item, merge?: string) {
  const s = currentScene();
  if (s) run({ op: "updateItem", scene: s.id, item }, merge);
}

export function updateScene(scene: Scene, merge?: string) {
  run({ op: "updateScene", scene }, merge);
}

/** Returns false (after a toast) when the JSON is not a project. */
export function loadProject(json: string, path: string | null = null, keepView = false): boolean {
  try {
    eng().load(json);
    if (!keepView) {
      app.sceneIndex = 0;
      app.frame = 0;
      app.selected = null;
    }
    app.filePath = path;
    eng().mark_saved();
    refresh();
    return true;
  } catch (e) {
    toast(`Could not open project: ${(e as Error).message ?? e}`, "error");
    return false;
  }
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;
export function toast(text: string, kind: "info" | "error" = "info") {
  app.toast = { text, kind };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (app.toast = null), kind === "error" ? 6000 : 2800);
}

// ---------------------------------------------------------------- autosave
// IndexedDB keeps the last session (including embedded audio) on both desktop and web.

const DB = "frameling";
function idb(): Promise<IDBDatabase> {
  return new Promise((res, rej) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => r.result.createObjectStore("kv");
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });
}
async function kvSet(key: string, val: string) {
  const db = await idb();
  await new Promise<void>((res, rej) => {
    const tx = db.transaction("kv", "readwrite");
    tx.objectStore("kv").put(val, key);
    tx.oncomplete = () => res();
    tx.onerror = () => rej(tx.error);
  });
}
async function kvGet(key: string): Promise<string | null> {
  const db = await idb();
  return new Promise((res) => {
    const r = db.transaction("kv").objectStore("kv").get(key);
    r.onsuccess = () => res((r.result as string) ?? null);
    r.onerror = () => res(null);
  });
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;
function scheduleAutosave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    if (engine) kvSet("autosave", engine.project_json()).catch(() => {});
  }, 800);
}
async function loadAutosave() {
  try {
    return await kvGet("autosave");
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------- helpers

export const secs = (frames: number) => frames / (app.project?.fps ?? 30);
export const frames = (s: number) => Math.round(s * (app.project?.fps ?? 30));
export const fmtTime = (f: number) => {
  const fps = app.project?.fps ?? 30;
  const s = f / fps;
  const m = Math.floor(s / 60);
  return `${m}:${(s - m * 60).toFixed(1).padStart(4, "0")}`;
};
