// Voices: built-in text-to-speech (desktop), microphone recording and the
// "add a line" flow that places dialogue on the scene like a dialogue track.

import { app, currentScene, run, toast } from "./store.svelte";
import { isDesktop } from "./platform";
import { decodeBytes, encodeWav, fromB64, mediaFromBytes, previewBytes } from "./audio";
import { uid } from "./defaults";
import type { Media, Voice } from "./types";

let cache: Voice[] | null = null;

/** Voices available on this machine (bundled neural voices first, then OS voices). */
export async function listVoices(refresh = false): Promise<Voice[]> {
  if (!isDesktop) return [];
  if (cache && !refresh) return cache;
  const { invoke } = await import("@tauri-apps/api/core");
  cache = await invoke<Voice[]>("tts_voices");
  return cache;
}

export const ttsAvailable = () => isDesktop;

// ---------------------------------------------------------------- built-in voice licence
// The Supertonic weights are OpenRAIL-M: its use restrictions have to be passed
// on to, and accepted by, the people using them. Accepted once per machine.
const TERMS_KEY = "frameling.supertonicTermsAccepted.v1";

export function termsAccepted(): boolean {
  try {
    return localStorage.getItem(TERMS_KEY) === "1";
  } catch {
    return false;
  }
}

export function acceptTerms() {
  try {
    localStorage.setItem(TERMS_KEY, "1");
  } catch {
    /* private mode: ask again next time */
  }
}

/** Licence summary and the path of the full licence file. */
export async function voiceTerms(): Promise<{ text: string; licence: string | null }> {
  const { invoke } = await import("@tauri-apps/api/core");
  const [text, licence] = await invoke<[string, string | null]>("tts_terms");
  return { text, licence };
}

/** Languages the built-in voices speak. */
export const BUILT_IN_LANGS: [string, string][] = [
  ["en", "English"], ["ar", "Arabic"], ["bg", "Bulgarian"], ["hr", "Croatian"], ["cs", "Czech"], ["da", "Danish"],
  ["nl", "Dutch"], ["et", "Estonian"], ["fi", "Finnish"], ["fr", "French"], ["de", "German"], ["el", "Greek"],
  ["hi", "Hindi"], ["hu", "Hungarian"], ["id", "Indonesian"], ["it", "Italian"], ["ja", "Japanese"], ["ko", "Korean"],
  ["lv", "Latvian"], ["lt", "Lithuanian"], ["pl", "Polish"], ["pt", "Portuguese"], ["ro", "Romanian"], ["ru", "Russian"],
  ["sk", "Slovak"], ["sl", "Slovenian"], ["es", "Spanish"], ["sv", "Swedish"], ["tr", "Turkish"], ["uk", "Ukrainian"],
  ["vi", "Vietnamese"],
];

/** `supertonic:F1@de` -> { base: "supertonic:F1", lang: "de" }. */
export function splitVoiceId(id: string): { base: string; lang: string | null } {
  const at = id.indexOf("@");
  return at < 0 ? { base: id, lang: null } : { base: id.slice(0, at), lang: id.slice(at + 1) };
}

export async function synthBytes(text: string, voice: string, speed: number): Promise<ArrayBuffer> {
  if (/^(supertonic|piper):/.test(voice) && !termsAccepted()) throw new Error("Accept the built-in voice terms first (Voice panel).");
  const { invoke } = await import("@tauri-apps/api/core");
  const b64 = await invoke<string>("tts_synthesize", { text, voice, speed });
  return fromB64(b64);
}

export async function previewVoice(voice: string, speed: number, text: string) {
  try {
    await previewBytes(await synthBytes(text, voice, speed));
  } catch (e) {
    toast(String(e), "error");
  }
}

/** Put a line on the current scene: after the previous line, scene grows to fit. */
export function placeLine(media: Media, speaker: string | null): boolean {
  const s = currentScene();
  if (!s) return false;
  const clipId = uid("line");
  const ok = run({ op: "addLine", scene: s.id, clipId, media, speaker, start: null });
  if (ok) {
    const clip = currentScene()?.audio.find((c) => c.id === clipId);
    if (clip) app.frame = clip.start;
  }
  return ok;
}

/** Speak `text` with a voice and add it as a line for `speaker`. */
export async function sayLine(text: string, voice: string, speed: number, speaker: string | null): Promise<boolean> {
  const bytes = await synthBytes(text, voice, speed);
  const name = text.length > 28 ? `${text.slice(0, 28)}…` : text;
  const media = await mediaFromBytes(bytes, `${name}.wav`, "audio/wav", { kind: "tts", text, voice, speed });
  return placeLine(media, speaker);
}

/** Re-generate a typed line with another voice/speed, keeping its place. */
export async function redoLine(clipId: string, text: string, voice: string, speed: number) {
  const s = currentScene();
  const clip = s?.audio.find((c) => c.id === clipId);
  if (!s || !clip) return;
  const bytes = await synthBytes(text, voice, speed);
  const name = text.length > 28 ? `${text.slice(0, 28)}…` : text;
  const media = await mediaFromBytes(bytes, `${name}.wav`, "audio/wav", { kind: "tts", text, voice, speed });
  run({
    op: "batch",
    commands: [
      { op: "removeAudio", scene: s.id, id: clip.id },
      { op: "addLine", scene: s.id, clipId: uid("line"), media, speaker: clip.speaker, start: clip.start },
    ],
  });
}

// ---------------------------------------------------------------- microphone

let recorder: MediaRecorder | null = null;
let chunks: Blob[] = [];
let stream: MediaStream | null = null;

export async function startRecording(): Promise<void> {
  if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
    throw new Error("recording isn't supported here");
  }
  stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } });
  chunks = [];
  recorder = new MediaRecorder(stream);
  recorder.ondataavailable = (e) => e.data.size && chunks.push(e.data);
  recorder.start();
}

/** Stop and return the take as WAV bytes. */
export async function stopRecording(): Promise<ArrayBuffer> {
  const r = recorder;
  if (!r) throw new Error("not recording");
  const done = new Promise<void>((res) => (r.onstop = () => res()));
  r.stop();
  await done;
  stream?.getTracks().forEach((t) => t.stop());
  recorder = null;
  stream = null;
  const blob = new Blob(chunks, { type: r.mimeType || "audio/webm" });
  const decoded = await decodeBytes(await blob.arrayBuffer());
  return encodeWav(trimSilence(decoded));
}

/** Drop leading/trailing silence so the line starts when the speaker does. */
function trimSilence(buf: AudioBuffer): AudioBuffer {
  const d = buf.getChannelData(0);
  const th = 0.02;
  let a = 0;
  let b = d.length - 1;
  while (a < b && Math.abs(d[a]) < th) a++;
  while (b > a && Math.abs(d[b]) < th) b--;
  const pad = Math.floor(buf.sampleRate * 0.08);
  a = Math.max(0, a - pad);
  b = Math.min(d.length - 1, b + pad);
  if (b - a < buf.sampleRate * 0.1) return buf;
  const out = new AudioBuffer({ length: b - a + 1, numberOfChannels: buf.numberOfChannels, sampleRate: buf.sampleRate });
  for (let c = 0; c < buf.numberOfChannels; c++) out.copyToChannel(buf.getChannelData(c).subarray(a, b + 1), c);
  return out;
}

export async function addRecordedLine(wav: ArrayBuffer, speaker: string | null, speakerName: string) {
  const media = await mediaFromBytes(wav, `${speakerName} (recorded).wav`, "audio/wav", { kind: "recording" });
  return placeLine(media, speaker);
}
