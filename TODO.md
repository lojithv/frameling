# Improvements to do

A working list after the creator essentials landed (see ROADMAP.md). Ranked by
impact: bugs that give a wrong video first, then things that waste the user's
time, then polish. Each item is ticked off when it is merged to main, with the
pull request that did it. Items ticked here are done on branch
fix/post-merge-polish.

Legend: **[bug]** wrong output, **[ux]** friction, **[perf]** speed,
**[polish]** looks or code health, **[feature]** missing piece.

## Now (P0: wrong output)

- [x] **[bug] Script captions land in the wrong scene.** A `> Ten minutes
      later...` line written after a scene's dialogue is shown over that whole
      scene from its first frame, instead of opening the next scene. Captions
      should be timed cards: one written before a scene's first line opens that
      scene, one written after its last line opens the next scene (or closes the
      movie). The parser comment also says "bottom" while the card is drawn at
      the top.
- [x] **[bug] Exports drop everything from loaded asset packs.** Both export
      paths build a fresh library: the browser export starts a second engine
      without the packs, and the desktop export renders with the built-in
      library only. Props, backgrounds and actions from a pack the user loaded
      vanish from the MP4.
- [x] **[bug] Export engine leaks.** The second engine made for an export is
      only freed after a successful desktop export, not after a browser export,
      an error, a cancel, or closing the save dialog. Each export leaks a full
      copy of the project (including its audio and images) in WebAssembly memory.

## Next (P1: friction)

- [x] **[ux] Loaded packs are forgotten on restart.** A pack has to be loaded
      again every session, and a project that uses one opens with missing items.
      Remember loaded packs and load them again on start.
- [x] **[ux] Script to video cannot be undone in one step.** It replaces the
      project after a confirm; make the previous project come back with Undo, or
      at least offer to save first.
- [x] **[ux] Script errors are terse.** Unknown backgrounds and unrecognised
      tags are only logged. Show them in the script dialog before generating,
      with the list of valid backgrounds and tags.

## Later (P2: polish and speed)

- [x] **[polish] Compiler warnings.** Two needless `mut` closures (rig.rs,
      stick.rs) and an unused `repository` workspace field.
- [ ] **[perf] Draft export still renders every frame at full cost.** A draft
      could also render at half the frame rate.
- [x] **[feature] Thumbnail picker.** Choose the thumbnail frame in the export
      dialog with a small scrubber and preview, instead of "the frame at the
      playhead".

## Needs a person at the desktop app

These were verified through the same engine and CLI code paths, but not by
clicking through the installed app:

- [ ] Script to video with voices, run from the editor's dialog.
- [ ] Export with "Save a thumbnail" and "Quick draft" ticked: check the .png
      and the half-size .mp4 appear next to the video.
