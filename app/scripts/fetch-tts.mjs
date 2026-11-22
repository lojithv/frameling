// Downloads the built-in text-to-speech engine (Piper, MIT) and a curated set of
// openly licensed Piper voices into src-tauri/resources/tts so they ship inside
// the desktop app. Run once before `npm run tauri build`:
//
//   npm run fetch-tts                # engine for this machine + default voices
//   npm run fetch-tts -- --target windows-x64 --voices en_US-ljspeech-medium
//
// Piper runs as a separate process. Its phonemiser (espeak-ng, GPL-3.0) ships
// inside Piper's own folder and is not linked into Frameling.
import { execSync } from "node:child_process";
import { createWriteStream, existsSync, mkdirSync, writeFileSync, rmSync, readdirSync, renameSync, chmodSync } from "node:fs";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../src-tauri/resources/tts");
const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);

const PIPER_RELEASE = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2";
const ASSETS = {
  "linux-x64": "piper_linux_x86_64.tar.gz",
  "linux-arm64": "piper_linux_aarch64.tar.gz",
  "macos-x64": "piper_macos_x64.tar.gz",
  "macos-arm64": "piper_macos_aarch64.tar.gz",
  "windows-x64": "piper_windows_amd64.zip",
};
const host = `${process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux"}-${process.arch === "arm64" ? "arm64" : "x64"}`;
const target = opt("--target", host);

// Voices whose training data allows redistribution, with the credit the UI shows.
// Multi-speaker models get friendly names for a handful of speakers (preview them
// in the app and edit voices/frameling-voices.json to taste).
const VOICES = {
  "en_US-ljspeech-medium": { lang: "en/en_US/ljspeech/medium", credit: "LJ Speech (public domain)", name: "Linda (US)" },
  "en_US-libritts_r-medium": {
    lang: "en/en_US/libritts_r/medium",
    credit: "LibriTTS-R, CC BY 4.0",
    speakers: { 0: "Alex (US)", 23: "Sam (US)", 67: "Jordan (US)", 110: "Riley (US)", 154: "Casey (US)", 205: "Morgan (US)", 301: "Taylor (US)", 450: "Jamie (US)", 600: "Quinn (US)", 777: "Avery (US)" },
  },
  "en_GB-vctk-medium": {
    lang: "en/en_GB/vctk/medium",
    credit: "CSTR VCTK, CC BY 4.0",
    speakers: { 0: "Oliver (UK)", 5: "Harriet (UK)", 12: "Fergus (UK)", 30: "Imogen (UK)", 55: "Rhys (UK)", 80: "Isla (UK)" },
  },
};
const wanted = opt("--voices", Object.keys(VOICES).join(",")).split(",");
const HF = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

async function download(url, file) {
  if (existsSync(file)) return console.log(`✓ ${path.basename(file)} (cached)`);
  console.log(`↓ ${url}`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status} ${url}`);
  await pipeline(Readable.fromWeb(res.body), createWriteStream(file + ".part"));
  renameSync(file + ".part", file);
}

mkdirSync(path.join(root, "voices"), { recursive: true });
const cache = path.resolve(here, "../.tts-cache"); // outside resources so it is not bundled
mkdirSync(cache, { recursive: true });

// Engine
const asset = ASSETS[target];
if (!asset) throw new Error(`no Piper build for ${target}`);
const archive = path.join(cache, asset);
await download(`${PIPER_RELEASE}/${asset}`, archive);
rmSync(path.join(root, "piper"), { recursive: true, force: true });
execSync(`tar -xf "${archive}" -C "${root}"`, { stdio: "inherit" }); // bsdtar on Windows handles .zip too
const bin = path.join(root, "piper", target.startsWith("windows") ? "piper.exe" : "piper");
if (!existsSync(bin)) throw new Error(`piper binary missing after extracting ${asset}`);
if (!target.startsWith("windows")) chmodSync(bin, 0o755);

// Voices
const curated = {};
for (const id of wanted) {
  const v = VOICES[id];
  if (!v) throw new Error(`unknown voice ${id}; known: ${Object.keys(VOICES).join(", ")}`);
  for (const ext of [".onnx", ".onnx.json"]) await download(`${HF}/${v.lang}/${id}${ext}`, path.join(root, "voices", id + ext));
  curated[id] = { credit: v.credit, ...(v.name ? { name: v.name } : {}), ...(v.speakers ? { speakers: v.speakers } : {}) };
}
writeFileSync(path.join(root, "voices", "frameling-voices.json"), JSON.stringify(curated, null, 2));
writeFileSync(
  path.join(root, "voices", "CREDITS.txt"),
  Object.entries(curated).map(([id, c]) => `${id}: ${c.credit}`).join("\n") +
    "\nVoice models: Piper (https://github.com/rhasspy/piper), MIT.\n",
);
console.log(`Built-in TTS ready for ${target}: ${readdirSync(path.join(root, "voices")).filter((f) => f.endsWith(".onnx")).length} voice models.`);
