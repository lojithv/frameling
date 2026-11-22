// Library thumbnails rendered by the engine itself (so they always match the stage), cached as data URLs.

import { eng } from "./store.svelte";
import type { Item } from "./types";

const cache = new Map<string, string>();

/** Wrap engine pixels (RGBA) as ImageData without copying. */
export function toImageData(px: Uint8Array, w: number, h: number): ImageData {
  return new ImageData(new Uint8ClampedArray(px.buffer as ArrayBuffer, px.byteOffset, px.byteLength), w, h);
}

function toUrl(px: Uint8Array, w: number, h: number): string {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  c.getContext("2d")!.putImageData(toImageData(px, w, h), 0, 0);
  return c.toDataURL();
}

export function itemThumb(key: string, item: Partial<Item> & { kind: Item["kind"] }, w = 96, h = 96, frame = 0): string {
  const k = `${key}:${w}x${h}:${frame}`;
  let url = cache.get(k);
  if (!url) {
    const px = eng().render_item_preview(JSON.stringify({ id: "thumb", ...item }), w, h, frame);
    url = toUrl(px, w, h);
    cache.set(k, url);
  }
  return url;
}

export function backgroundThumb(id: string, w = 160, h = 90): string {
  const k = `bg:${id}:${w}`;
  let url = cache.get(k);
  if (!url) {
    url = toUrl(eng().render_background_preview(id, w, h), w, h);
    cache.set(k, url);
  }
  return url;
}

export function sceneThumb(sceneIndex: number, rev: number, w = 128, h = 72): string {
  const px = eng().render_scene(sceneIndex, 0, w, h);
  return toUrl(px, w, h) + `#${rev}`;
}

export function clearThumbs() {
  cache.clear();
}
