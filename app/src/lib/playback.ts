// Real-time preview playback. Preview may skip frames on a slow machine;
// export never does, because it renders by frame number.

import { app, currentScene, eng } from "./store.svelte";
import { playEvents, stopAudio } from "./audio";
import type { AudioEvent } from "./types";

let raf = 0;

export function play(mode: "scene" | "all" = app.playMode) {
  if (app.playing) stop();
  const p = app.project!;
  app.playMode = mode;
  app.playing = true;
  app.selected = app.selected; // keep selection
  const fps = p.fps;
  let start: number;
  let total: number;
  let events: AudioEvent[];
  if (mode === "all") {
    total = eng().total_frames();
    const starts = sceneStarts();
    start = starts[app.sceneIndex] + app.frame;
    if (start >= total - 1) start = 0;
    events = JSON.parse(eng().audio_events());
  } else {
    const s = currentScene()!;
    total = s.duration;
    start = app.frame >= total - 1 ? 0 : app.frame;
    events = s.audio
      .filter((c) => p.media[c.media])
      .map((c) => {
        const m = p.media[c.media];
        const avail = Math.max(0, m.durationFrames - c.trimStart);
        const frames = Math.min(c.length ?? avail, avail, s.duration - c.start);
        return { media: c.media, at: c.start, trim: c.trimStart, frames, volume: c.volume };
      })
      .filter((e) => e.frames > 0);
  }
  playEvents(events, start);
  const t0 = performance.now();
  const tick = () => {
    if (!app.playing) return;
    const f = start + Math.floor(((performance.now() - t0) / 1000) * fps);
    if (f >= total) {
      stop();
      return;
    }
    if (mode === "all") {
      app.globalFrame = f;
      const loc = eng().locate(f);
      if (loc.length) {
        app.sceneIndex = loc[0];
        app.frame = loc[1];
      }
    } else {
      app.frame = f;
    }
    raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);
}

export function stop() {
  app.playing = false;
  cancelAnimationFrame(raf);
  stopAudio();
}

export function toggle() {
  if (app.playing) stop();
  else play();
}

export function sceneStarts(): number[] {
  const out: number[] = [];
  let t = 0;
  for (const s of app.project?.scenes ?? []) {
    out.push(t);
    t += Math.max(1, s.duration);
  }
  return out;
}
