// Asset packs the user loaded (props, backgrounds, actions, character parts).
// The editor's engine gets them as they load; exports get them too (each
// export renders with an engine of its own), and they are remembered in this
// browser profile (or the desktop app's) so they load again on the next start.

import type { Engine } from "../engine/frameling.js";

const KEY = "frameling.packs";

/** Pack JSON by pack id, in load order. */
const loaded = new Map<string, string>();

function remembered(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}

/** Remember the loaded packs. False when storage is full or blocked. */
function remember(): boolean {
  try {
    localStorage.setItem(KEY, JSON.stringify([...loaded.values()]));
    return true;
  } catch {
    return false;
  }
}

/** Load a pack into an engine and keep it for exports and the next start. */
export function addPack(engine: Engine, json: string): { id: string; remembered: boolean } {
  const id = engine.add_pack(json);
  loaded.delete(id); // a newer version of the same pack replaces the old one
  loaded.set(id, json);
  return { id, remembered: remember() };
}

/** Load the packs remembered from earlier sessions. Packs that no longer load are dropped. */
export function restorePacks(engine: Engine): string[] {
  const ids: string[] = [];
  for (const json of remembered()) {
    try {
      const id = engine.add_pack(json);
      loaded.set(id, json);
      ids.push(id);
    } catch {
      /* skip a pack this version can't read */
    }
  }
  remember();
  return ids;
}

/** Every loaded pack's JSON, for engines made after the fact (exports). */
export function packJsons(): string[] {
  return [...loaded.values()];
}

/** Give an export engine the same packs as the editor's. */
export function applyPacks(engine: Engine) {
  for (const json of loaded.values()) engine.add_pack(json);
}
