// "My characters": looks saved from the character creator, kept in this
// browser profile (or the desktop app's) so they can be reused in any project.

import type { Look } from "./types";

export interface MyCharacter {
  id: string;
  name: string;
  look: Look;
  savedAt: number;
}

const KEY = "frameling.myCharacters";

export function loadMyCharacters(): MyCharacter[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((c) => c && typeof c.name === "string" && c.look) : [];
  } catch {
    return [];
  }
}

function write(list: MyCharacter[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* storage full or blocked */
  }
}

/** Save (or replace, by name) a character; returns the new list, newest first. */
export function saveMyCharacter(name: string, look: Look): MyCharacter[] {
  const list = loadMyCharacters().filter((c) => c.name !== name);
  const next = [{ id: `my-${Date.now().toString(36)}`, name, look, savedAt: Date.now() }, ...list].slice(0, 200);
  write(next);
  return next;
}

export function deleteMyCharacter(id: string): MyCharacter[] {
  const next = loadMyCharacters().filter((c) => c.id !== id);
  write(next);
  return next;
}
