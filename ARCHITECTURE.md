# Architecture

## Layers

1. **Document** (`core/model.rs`): `Project → Scene[] → Item[] + AudioClip[]`, plus embedded `Media`.
   Plain serde JSON, camelCase, versioned (`version`, `migrate()`).
2. **Commands** (`core/command.rs`): the only way the document changes. Every UI action,
   plugin and script sends a serialisable `Command`. `Editor` applies them to a copy and keeps
   snapshots for undo (media is behind `Arc`, so snapshots are cheap). A `merge` key collapses a
   drag gesture into one undo step.
3. **Library** (`core/pack.rs`): props, backgrounds and actions loaded from JSON packs. Colours
   can reference per-item slots (`$0`, `$1d` = darker), which gives recolouring for free.
4. **Rig** (`core/rig.rs`): characters are drawn procedurally from a `Look` and a `Pose`
   (joint angles). Actions are keyframed poses in packs, sampled with smoothstep and looped.
5. **Keyframes** (`core/anim/`): one engine for every animatable property. Items and scenes carry
   `tracks` (path → keys, e.g. `transform.x`, `bone.forearm_l.rot`, `look.skin`, `outfit`,
   `camera.zoom`). `track.rs` holds keys, interpolation (constant, linear, eases, back, elastic,
   bounce, CSS bezier) and binary-search sampling; `path.rs` is the path grammar and value
   validation. Keyed values override static fields, so unkeyed projects render unchanged.
6. **Evaluator** (`core/eval.rs`): `evaluate(project, lib, frame) -> DisplayList`. Resolves the
   scene, camera, motion paths, enter/exit effects, actions, blinking (hashed per item, no RNG),
   lip-sync level from the audio envelope, text layout and transitions.
7. **Display list** (`core/display.rs`): groups (transform + opacity), fills (solid / linear
   gradient) and strokes. Text is converted to glyph outlines in core, so renderers don't need fonts.
8. **Renderer** (`render`): tiny-skia, CPU, deterministic. Runs in WASM (preview) and natively (export).
9. **Export** (`export`): renders frames in parallel batches, writes them in order to FFmpeg's
   stdin as raw RGBA, mixes audio with `atrim/adelay/amix` placed by frame number.

## Why the UI runs the engine as WASM, even on desktop

One engine build serves both targets, so desktop and web behave identically, and interactive
editing never crosses an IPC boundary. The native side only adds what browsers can't do well:
real file paths and fast multi-threaded export with FFmpeg.

## Determinism rules

- Time is an integer frame index. No wall clock, no `Math.random`.
- Anything that looks random (blink timing) is hashed from the item id.
- Lip sync uses a loudness envelope computed once at import and stored in the project.
- Audio is placed at `frame / fps` seconds, never "when the player gets there".

## Next steps

See [ROADMAP.md](ROADMAP.md) for the full list.

- GPU renderer (Vello on wgpu) behind the same display list, for 4K and heavy scenes.
- WASM plugins (Extism) that receive commands and can register effects and importers.
- Phoneme-based lip sync (Rhubarb), using the durations Supertonic already predicts.
- Zip project format (`.frameling` = zip of JSON + media) for large audio.
