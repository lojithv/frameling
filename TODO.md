# Improvements to do

A working list after the creator essentials landed (see ROADMAP.md). Ranked by
impact: bugs that give a wrong video first, then things that waste the user's
time, then polish. Each item is ticked off when it is merged to main, with the
pull request that did it. Rounds 1 and 2 landed in PRs 1 and 2; round 3
is on branch fix/editing-round-3.

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
- [x] **[perf] Draft export still renders every frame at full cost.** A draft
      could also render at half the frame rate.
- [x] **[feature] Thumbnail picker.** Choose the thumbnail frame in the export
      dialog with a small scrubber and preview, instead of "the frame at the
      playhead".

## Round 3 (editing and desktop fixes)

- [x] **[bug] Arrow-key nudge does nothing on animated items.** It moved the
      base place, which position keys override. It is now an edit at the
      playhead like a drag (keys it), and a burst of presses is one undo step.
- [x] **[bug] Duplicate loses its offset on animated items.** Ctrl+D and the
      Inspector's Duplicate shifted only the base place; the copy now moves its
      position keys and old-style movement too, and sits just above the original.
- [x] **[bug] A restored session forgets its file.** After a restart, Save asked
      where to save again and the agent live-sync stopped. The autosave now
      keeps the file path and saved state; a clean session reloads the file (in
      case an agent changed it while the app was closed), a session with
      unsaved changes keeps them and says if the file changed.
- [x] **[bug] Open discards unsaved changes without asking** (Ctrl+O and
      File → Open…). It now asks, like New and Recent projects.
- [x] **[bug] Desktop export fails when the app is opened from Finder.** macOS
      gives Finder-launched apps a PATH without Homebrew, so FFmpeg was not
      found. The exporter now also looks in /opt/homebrew/bin, /usr/local/bin
      and similar places.
- [x] **[feature] Copy, cut and paste items** (Ctrl/Cmd+C, X, V), including
      into another scene. Animation comes along; an item that starts after the
      target scene ends is moved to start at the playhead.

## Needs a person at the desktop app

These were verified through the same engine and CLI code paths, but not by
clicking through the installed app:

- [ ] Script to video with voices, run from the editor's dialog.
- [ ] Export with "Save a thumbnail" and "Quick draft" ticked: check the .png
      and the half-size .mp4 appear next to the video.
