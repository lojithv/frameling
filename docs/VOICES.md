# Voices: how GoAnimate did it, and how Frameling does it

## The GoAnimate / Vyond model (behaviour only)

1. **Select a character, then "make it talk".** Dialogue belongs to a character; whoever
   is selected becomes the speaker.
2. **Three ways to get the audio**: type text and pick a **text-to-speech voice** (a long
   list of named voices by language and accent), **record** with the microphone, or **upload**
   a sound file.
3. **TTS ran on the server.** The studio sent the text and voice id to GoAnimate's backend,
   which called commercial TTS vendors and returned an MP3. Wrapper: Offline recreates that
   API locally; because the original voices are commercial, its TTS has to reach out to
   third-party online services rather than ship voices itself.
4. **The line lands on the timeline** in the scene's sound track. The scene is **lengthened
   to fit** the dialogue, and lines play one after another.
5. **Lip sync is automatic and loudness-driven.** While the line plays, the character's mouth
   opens and closes with the volume of the audio, and the body plays a talking animation.
6. **You can re-time** a line by dragging it, and swap the voice by re-generating it.

## Frameling's version

| | GoAnimate | Frameling |
|---|---|---|
| Who speaks | selected character | selected character (`speaker` on the clip) |
| Default voice | per line | per character (`voice`, `voiceSpeed`), used for every new line |
| TTS | cloud vendors | **built in and offline**: Supertonic 3 neural voices (10 voices, 31 languages) bundled with the desktop app, OS voices as a fallback |
| Record | Flash mic | microphone, silence trimmed, saved as WAV |
| Upload | MP3 | any format the platform decodes |
| Placement | after the last line, scene grows | `addLine` command: after the last spoken line (+0.2 s), scene grows to fit (+0.5 s) |
| Lip sync | volume | per-frame loudness envelope stored in the project, so preview and every export match exactly |
| Talking body | talk animation | idle characters switch to the `talk` action while speaking |
| Re-voice | regenerate | ↻ re-speaks a typed line with the current voice and speed, in the same place |
| Agents / scripts | no | `say_line` MCP tool, `frameling say`, and screenplays (`script_to_video`) |

### Built-in voices

`npm run fetch-tts` downloads Supertone's **Supertonic 3** model (ONNX, about 400 MB, pinned to
one revision) into `app/src-tauri/resources/tts/supertonic`, and the installer ships it. No
internet is needed after that. It runs in-process on ONNX Runtime (the `ort` crate), on the CPU,
several times faster than real time.

- **Voices**: ten voice styles (`supertonic:F1`…`F5`, `M1`…`M5`). The friendly names are
  Frameling's own.
- **Languages**: 31. A character's language is a suffix on its voice id (`supertonic:M2@de`);
  the Voice panel has a language menu next to the voice menu. No suffix means English.
- **Expression tags**: `<laugh>`, `<breath>` and `<sigh>` can be typed inline in a line.
- **Repeatable**: the starting noise is seeded from the text, voice and speed, so re-speaking a
  line gives the same audio.
- **Speed**: Frameling's 1.0 is Supertonic's natural pace (its `speed=1.05`).
- **Old projects**: `piper:` voice ids from v0.2 are re-voiced with a Supertonic voice picked
  from the old id, so characters keep distinct voices.

### Licences (read before redistributing)

| Part | Licence | What it asks of us |
|---|---|---|
| Frameling code, including the Supertonic pipeline in `crates/tts` | MIT OR Apache-2.0 | nothing new. The pipeline follows Supertone's MIT sample code. |
| ONNX Runtime (linked into the app) | MIT | keep its notice in the app's third-party notices |
| **Supertonic 3 weights** (`resources/tts/supertonic`) | **BigScience OpenRAIL-M** | see below |

OpenRAIL-M is not an OSI open-source licence. Shipping the weights inside the installer is
"distribution" under it, so Frameling must:

1. include the licence text (`LICENSE` sits next to the weights, with a `NOTICE.txt`);
2. pass the use restrictions in paragraph 5 and Attachment A on to users as an enforceable
   term, and tell them the model is subject to them. The app shows a summary and asks the user
   to agree before the built-in voices are first used. The CLI and the MCP `list_voices` tool
   print the same terms;
3. keep the files unmodified, or mark any changes prominently.

The restrictions that matter most for a video maker: no impersonating a real person or
making deepfakes without consent, and **generated speech must be disclosed as
machine-generated** where it is published. Users are accountable for their output.

Supertone archived its GitHub repo on 9 September 2026 (read-only, no further fixes). The
Hugging Face model `Supertone/supertonic-3` is still downloadable, and the script pins a
revision, so builds stay reproducible. Mirror the files yourself if that repo ever disappears.

The web build has no typed-line TTS yet (record and upload work). Supertonic can run in the
browser with onnxruntime-web (WebGPU or WASM), but the 400 MB download makes that a separate
decision.
