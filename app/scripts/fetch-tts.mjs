// Downloads the built-in text-to-speech model (Supertone's Supertonic 3, ONNX)
// into src-tauri/resources/tts/supertonic so it ships inside the desktop app.
// Run once before `npm run tauri build` (or `npm run tauri dev`):
//
//   npm run fetch-tts
//   npm run fetch-tts -- --voices F1,M1      # fewer voice styles (the model is the bulk)
//
// Supertonic runs in-process through ONNX Runtime (the `ort` crate), so there
// is no separate engine binary to download.
//
// LICENCE: the model weights are under the BigScience OpenRAIL-M licence, not
// MIT/Apache. Its use restrictions (Attachment A) travel with the weights: the
// LICENSE file is copied next to them and the app asks the user to accept it
// before first use. See docs/VOICES.md.
import { createWriteStream, existsSync, mkdirSync, writeFileSync, renameSync, copyFileSync, statSync, rmSync } from "node:fs";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const resources = path.resolve(here, "../src-tauri/resources/tts");
const root = path.join(resources, "supertonic");
const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);

// Pinned so a line re-voiced next year sounds like it did today.
const REVISION = "724fb5abbf5502583fb520898d45929e62f02c0b";
const HF = `https://huggingface.co/Supertone/supertonic-3/resolve/${REVISION}`;
const MODEL_FILES = [
  "onnx/tts.json",
  "onnx/unicode_indexer.json",
  "onnx/duration_predictor.onnx",
  "onnx/text_encoder.onnx",
  "onnx/vector_estimator.onnx",
  "onnx/vocoder.onnx",
  "LICENSE",
];
const ALL_VOICES = ["F1", "F2", "F3", "F4", "F5", "M1", "M2", "M3", "M4", "M5"];
const voices = opt("--voices", ALL_VOICES.join(",")).split(",").map((v) => v.trim().toUpperCase());
for (const v of voices) if (!ALL_VOICES.includes(v)) throw new Error(`unknown voice ${v}; known: ${ALL_VOICES.join(", ")}`);

const cache = path.resolve(here, "../.tts-cache", REVISION); // outside resources so it is not bundled

async function download(rel) {
  const file = path.join(cache, rel);
  mkdirSync(path.dirname(file), { recursive: true });
  if (existsSync(file)) {
    console.log(`✓ ${rel} (cached)`);
    return file;
  }
  console.log(`↓ ${rel}`);
  const res = await fetch(`${HF}/${rel}`);
  if (!res.ok) throw new Error(`${res.status} ${HF}/${rel}`);
  await pipeline(Readable.fromWeb(res.body), createWriteStream(file + ".part"));
  renameSync(file + ".part", file);
  return file;
}

function place(rel, file) {
  const dest = path.join(root, rel);
  mkdirSync(path.dirname(dest), { recursive: true });
  if (!existsSync(dest) || statSync(dest).size !== statSync(file).size) copyFileSync(file, dest);
}

// Earlier versions shipped Piper here; drop it so it is not bundled any more.
rmSync(path.join(resources, "piper"), { recursive: true, force: true });
rmSync(path.join(resources, "voices"), { recursive: true, force: true });

for (const rel of MODEL_FILES) place(rel, await download(rel));
rmSync(path.join(root, "voice_styles"), { recursive: true, force: true });
for (const v of voices) place(`voice_styles/${v}.json`, await download(`voice_styles/${v}.json`));

writeFileSync(
  path.join(root, "NOTICE.txt"),
  [
    "Supertonic 3 text-to-speech model",
    "Copyright (c) 2026 Supertone Inc.",
    `Source: https://huggingface.co/Supertone/supertonic-3 (revision ${REVISION})`,
    "",
    "The model files in this folder are licensed under the BigScience OpenRAIL-M License",
    "(see LICENSE). They are NOT covered by Frameling's MIT/Apache-2.0 licence.",
    "By using these voices you agree to the use restrictions in paragraph 5 and",
    "Attachment A of that licence. Among them: say clearly that generated speech is",
    "machine-generated, and do not impersonate a real person without their consent.",
    "The files are unmodified.",
    "",
  ].join("\n"),
);
console.log(`Built-in TTS ready: Supertonic 3 with ${voices.length} voice styles in ${root}`);
