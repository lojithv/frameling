// Audio import (with the lip-sync envelope) and preview playback.

import { app, eng } from "./store.svelte";
import type { AudioEvent, Media, MediaSource } from "./types";
import { uid } from "./defaults";

let ctx: AudioContext | null = null;
const buffers = new Map<string, AudioBuffer>();
let playing: AudioBufferSourceNode[] = [];

function audioCtx() {
  ctx ??= new AudioContext();
  return ctx;
}

function toBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function fromBase64(b64: string): ArrayBuffer {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out.buffer;
}

/**
 * Loudness per video frame, normalised to 0..1. Stored in the project so
 * lip sync is identical in preview and in every export.
 */
function envelope(buf: AudioBuffer, fps: number): number[] {
  const per = Math.max(1, Math.floor(buf.sampleRate / fps));
  const n = Math.ceil(buf.length / per);
  const chans = Array.from({ length: buf.numberOfChannels }, (_, c) => buf.getChannelData(c));
  const env: number[] = [];
  for (let f = 0; f < n; f++) {
    let sum = 0;
    const s0 = f * per;
    const s1 = Math.min(buf.length, s0 + per);
    for (let i = s0; i < s1; i++) {
      let v = 0;
      for (const ch of chans) v += ch[i];
      v /= chans.length;
      sum += v * v;
    }
    env.push(Math.sqrt(sum / Math.max(1, s1 - s0)));
  }
  // Normalise against a high percentile so one loud click doesn't flatten everything.
  const sorted = [...env].sort((a, b) => a - b);
  const ref = sorted[Math.floor(sorted.length * 0.95)] || 1e-6;
  return env.map((v) => Math.round(Math.min(1, v / ref) * 1000) / 1000);
}

/** Read an audio file into a Media record ready for the `addMedia` command. */
export async function importAudio(file: File): Promise<Media> {
  return mediaFromBytes(await file.arrayBuffer(), file.name, file.type || "audio/mpeg", { kind: "file" });
}

/** Decode audio bytes, measure them, and build a Media record (lip-sync envelope included). */
export async function mediaFromBytes(bytes: ArrayBuffer, name: string, mime: string, source: MediaSource): Promise<Media> {
  const decoded = await audioCtx().decodeAudioData(bytes.slice(0));
  const fps = app.project?.fps ?? 30;
  const id = uid("media");
  buffers.set(id, decoded);
  return {
    id,
    name,
    mime,
    data: toBase64(bytes),
    durationFrames: Math.ceil(decoded.duration * fps),
    envelope: envelope(decoded, fps),
    source,
  };
}

/** Play raw audio bytes once (voice previews). */
export async function previewBytes(bytes: ArrayBuffer) {
  stopAudio();
  const c = audioCtx();
  if (c.state === "suspended") await c.resume();
  const buf = await c.decodeAudioData(bytes.slice(0));
  const src = c.createBufferSource();
  src.buffer = buf;
  src.connect(c.destination);
  src.start();
  playing.push(src);
}

/** Play one media item from the project (line previews). */
export async function previewMedia(mediaId: string) {
  stopAudio();
  const c = audioCtx();
  if (c.state === "suspended") await c.resume();
  const buf = await getBuffer(mediaId);
  if (!buf) return;
  const src = c.createBufferSource();
  src.buffer = buf;
  src.connect(c.destination);
  src.start();
  playing.push(src);
}

export function fromB64(b64: string): ArrayBuffer {
  return fromBase64(b64);
}

/** Encode an AudioBuffer as 16-bit mono WAV (portable everywhere, FFmpeg included). */
export function encodeWav(buf: AudioBuffer): ArrayBuffer {
  const n = buf.length;
  const rate = buf.sampleRate;
  const chans = Array.from({ length: buf.numberOfChannels }, (_, c) => buf.getChannelData(c));
  const out = new ArrayBuffer(44 + n * 2);
  const v = new DataView(out);
  const str = (o: number, s: string) => [...s].forEach((ch, i) => v.setUint8(o + i, ch.charCodeAt(0)));
  str(0, "RIFF"); v.setUint32(4, 36 + n * 2, true); str(8, "WAVE"); str(12, "fmt ");
  v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
  v.setUint32(24, rate, true); v.setUint32(28, rate * 2, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true);
  str(36, "data"); v.setUint32(40, n * 2, true);
  for (let i = 0; i < n; i++) {
    let s = 0;
    for (const ch of chans) s += ch[i];
    s = Math.max(-1, Math.min(1, s / chans.length));
    v.setInt16(44 + i * 2, s < 0 ? s * 0x8000 : s * 0x7fff, true);
  }
  return out;
}

export async function decodeBytes(bytes: ArrayBuffer): Promise<AudioBuffer> {
  return audioCtx().decodeAudioData(bytes.slice(0));
}

export async function getBuffer(mediaId: string): Promise<AudioBuffer | null> {
  const cached = buffers.get(mediaId);
  if (cached) return cached;
  const b64 = eng().media_data(mediaId);
  if (!b64) return null;
  try {
    const buf = await audioCtx().decodeAudioData(fromBase64(b64));
    buffers.set(mediaId, buf);
    return buf;
  } catch {
    return null;
  }
}

/** Start playing the given events from `fromFrame` (same timeline the events use). */
export async function playEvents(events: AudioEvent[], fromFrame: number) {
  stopAudio();
  const fps = app.project?.fps ?? 30;
  const c = audioCtx();
  if (c.state === "suspended") await c.resume();
  const t0 = c.currentTime + 0.05;
  for (const ev of events) {
    const end = ev.at + ev.frames;
    if (end <= fromFrame) continue;
    const buf = await getBuffer(ev.media);
    if (!buf) continue;
    const src = c.createBufferSource();
    src.buffer = buf;
    const gain = c.createGain();
    gain.gain.value = ev.volume;
    src.connect(gain).connect(c.destination);
    const skip = Math.max(0, fromFrame - ev.at);
    const when = t0 + Math.max(0, ev.at - fromFrame) / fps;
    src.start(when, (ev.trim + skip) / fps, (ev.frames - skip) / fps);
    playing.push(src);
  }
}

export function stopAudio() {
  for (const s of playing) {
    try {
      s.stop();
    } catch {
      /* already stopped */
    }
  }
  playing = [];
}

/** Mix events into one buffer (used by the web exporter). */
export async function mixdown(events: AudioEvent[], totalFrames: number, fps: number): Promise<AudioBuffer | null> {
  if (!events.length) return null;
  const rate = 48000;
  const off = new OfflineAudioContext(2, Math.ceil((totalFrames / fps) * rate), rate);
  for (const ev of events) {
    const buf = await getBuffer(ev.media);
    if (!buf) continue;
    const src = off.createBufferSource();
    src.buffer = buf;
    const g = off.createGain();
    g.gain.value = ev.volume;
    src.connect(g).connect(off.destination);
    src.start(ev.at / fps, ev.trim / fps, ev.frames / fps);
  }
  return off.startRendering();
}
