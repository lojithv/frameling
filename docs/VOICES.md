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
| TTS | cloud vendors | **built in and offline**: Piper neural voices bundled with the desktop app, OS voices as a fallback |
| Record | Flash mic | microphone, silence trimmed, saved as WAV |
| Upload | MP3 | any format the platform decodes |
| Placement | after the last line, scene grows | `addLine` command: after the last spoken line (+0.2 s), scene grows to fit (+0.5 s) |
| Lip sync | volume | per-frame loudness envelope stored in the project, so preview and every export match exactly |
| Talking body | talk animation | idle characters switch to the `talk` action while speaking |
| Re-voice | regenerate | ↻ re-speaks a typed line with the current voice and speed, in the same place |
| Agents / scripts | no | `say_line` MCP tool, `frameling say`, and screenplays (`script_to_video`) |

### Built-in voices

`npm run fetch-tts` downloads Piper (MIT) and openly licensed voice models into
`app/src-tauri/resources/tts`, and the installer ships them. No internet is needed after
that. Defaults: LJ Speech (public domain), LibriTTS-R (CC BY 4.0, many speakers) and VCTK
(CC BY 4.0, British speakers). The friendly names for multi-speaker voices are placeholders;
preview them and edit `voices/frameling-voices.json`. Users can add any Piper voice (40+
languages) by dropping `.onnx` + `.onnx.json` into the app's `voices` data folder.

Piper runs as a separate process. Its phonemiser (espeak-ng) is GPL-3.0 and stays inside
Piper's folder, so Frameling's own code stays MIT/Apache.

The web build has no typed-line TTS yet (record and upload work). Running Piper in the
browser means compiling espeak-ng into the web bundle, which would put that bundle under
the GPL. See the roadmap.
