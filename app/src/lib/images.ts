// Picture import: any format the browser can decode becomes a PNG (at most
// 2048 px on its long side) embedded in the project, so the Rust renderer
// only needs PNG and the project stays one portable file.

import type { Media } from "./types";
import { uid } from "./defaults";

const MAX = 2048;

function toBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** Read an image file into a Media record ready for the `addMedia` command. */
export async function importImage(file: Blob, name = "Picture"): Promise<Media> {
  const bmp = await createImageBitmap(file);
  const k = Math.min(1, MAX / Math.max(bmp.width, bmp.height));
  const w = Math.max(1, Math.round(bmp.width * k));
  const h = Math.max(1, Math.round(bmp.height * k));
  const canvas = new OffscreenCanvas(w, h);
  const ctx = canvas.getContext("2d")!;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bmp, 0, 0, w, h);
  bmp.close();
  const png = await canvas.convertToBlob({ type: "image/png" });
  return {
    id: uid("image"),
    name: name.replace(/\.[a-z0-9]{2,5}$/i, ""),
    mime: "image/png",
    data: toBase64(await png.arrayBuffer()),
    durationFrames: 0,
    envelope: [],
    source: { kind: "file" },
    size: [w, h],
  };
}

/** A data URL to show a picture from the project in the UI. */
export const imageUrl = (m: Media) => `data:${m.mime};base64,${m.data}`;
