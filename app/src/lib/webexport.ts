// Browser export: render every frame with the WASM engine (deterministic,
// frame by frame, no screen recording), encode with WebCodecs via Mediabunny.

import {
  AudioBufferSource,
  BufferTarget,
  CanvasSource,
  Mp4OutputFormat,
  Output,
  QUALITY_HIGH,
  WebMOutputFormat,
  canEncodeAudio,
  canEncodeVideo,
} from "mediabunny";
import { eng } from "./store.svelte";
import { mixdown } from "./audio";
import type { AudioEvent } from "./types";
import { toImageData } from "./thumbs";
import type { ExportSettings } from "./platform";

/** Returns the video, or null if cancelled. `progress` returns false to cancel. */
export async function exportInBrowser(
  s: ExportSettings,
  progress: (done: number, label: string) => boolean,
): Promise<Blob | null> {
  if (typeof VideoEncoder === "undefined") {
    throw new Error("this browser can't encode video (WebCodecs missing). Try Chrome, Edge or the desktop app.");
  }
  const e = eng();
  const project = JSON.parse(e.project_view_json());
  const fps: number = project.fps;
  const total = e.total_frames();
  const w = s.width & ~1;
  const h = s.height & ~1;

  // Pick a container/codec combination this browser can actually encode.
  let mp4 = s.format === "mp4";
  let vcodec: "avc" | "vp9" = mp4 ? "avc" : "vp9";
  let acodec: "aac" | "opus" = mp4 ? "aac" : "opus";
  if (mp4 && !(await canEncodeVideo("avc", { width: w, height: h }))) {
    mp4 = false;
    vcodec = "vp9";
    acodec = "opus";
  }
  if (mp4 && !(await canEncodeAudio("aac"))) acodec = "opus"; // MP4 can hold Opus too
  if (!mp4 && !(await canEncodeVideo("vp9", { width: w, height: h }))) vcodec = "avc";

  const canvas = new OffscreenCanvas(w, h);
  const ctx = canvas.getContext("2d")!;
  const output = new Output({
    format: mp4 ? new Mp4OutputFormat({ fastStart: "in-memory" }) : new WebMOutputFormat(),
    target: new BufferTarget(),
  });
  const video = new CanvasSource(canvas, { codec: vcodec, bitrate: QUALITY_HIGH, keyFrameInterval: 2 });
  output.addVideoTrack(video, { frameRate: fps });

  const events: AudioEvent[] = JSON.parse(e.audio_events());
  let audio: AudioBufferSource | null = null;
  if (events.length) {
    audio = new AudioBufferSource({ codec: acodec, bitrate: QUALITY_HIGH });
    output.addAudioTrack(audio);
  }
  await output.start();

  if (audio) {
    if (!progress(0, "Mixing audio")) return cancel(output);
    const mixed = await mixdown(events, total, fps);
    if (mixed) await audio.add(mixed);
    audio.close();
  }

  for (let f = 0; f < total; f++) {
    const px = e.render_frame(f, w, h);
    ctx.putImageData(toImageData(px, w, h), 0, 0);
    await video.add(f / fps, 1 / fps);
    if (f % 5 === 0 && !progress(f, "Rendering")) return cancel(output);
  }
  video.close();
  progress(total, "Finishing");
  await output.finalize();
  const buf = (output.target as BufferTarget).buffer!;
  return new Blob([buf], { type: mp4 ? "video/mp4" : "video/webm" });
}

async function cancel(output: Output) {
  await output.cancel();
  return null;
}
