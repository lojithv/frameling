# Driving Frameling with an AI agent

Frameling is agent-native: everything the editor does is a JSON **command**, and the
`frameling mcp` server exposes the project, the asset library, the voices, rendering and
export to any MCP client (Claude Desktop, Claude Code, Cursor, ...). The desktop app
reloads the project live while an agent edits the same file.

## Connect

```json
{
  "mcpServers": {
    "frameling": { "command": "frameling", "args": ["mcp", "/path/to/movie.frameling"] }
  }
}
```

Claude Code: `claude mcp add frameling -- frameling mcp ~/movie.frameling`.
The file is created if it doesn't exist and saved after every change. Open the same file
in the desktop app to watch the agent work.

## Tools

| tool | what it does |
|---|---|
| `get_project` | Scenes, items, lines and timings as a readable outline (`detail: "full"` for JSON) |
| `get_library` | Props, backgrounds, actions and the character options |
| `list_voices` | Text-to-speech voices on this machine |
| `apply_commands` | Apply a list of commands as one undoable step |
| `say_line` | Make a character speak a line (TTS, lip-synced, placed after the previous line) |
| `script_to_video` | Build a whole project from a screenplay (replaces the current one) |
| `render_frame` | See a frame as a PNG image (by scene + seconds, or global frame) |
| `export_video` | Render MP4/WebM to a path |
| `undo` / `redo` | History |
| `new_project` | Start blank or from the demo |
| `set_keys` | Keyframe any property over time (see Keyframes below) |
| `delete_keys` | Delete keys by frame/second, by range, a whole track, or every track on an item |
| `list_tracks` | Show an item's (or scene's) tracks, keys, outfits and the path grammar |
| `set_interpolation` | Change how keys ease into the next key |
| `define_outfit` | Name a costume on a character, for `outfit` keys to switch to |
| `list_part_categories` | Character creator categories, part counts and colour fields |
| `search_parts` | Find character parts by words ("chef moustache", "winter", "podcast") |
| `create_character` | Add a character from a recipe of part ids and colours |
| `update_character` | Change some parts or colours of an existing character |
| `randomize_character` | Random look (seeded), with `keep` locks and pinned parts; also makes crowds |
| `preview_character` | See one character on its own as a PNG |
| `import_parts_pack` | Load a part pack (`parts[]`), e.g. custom hats or held props |
| `list_poses` | The pose library (stand, sit, kneel, point, phone call, lying, ...) and saved poses |
| `add_pose_strip` | Hold a pose for a time with blends; `mask: "upper"` gestures over a walk |
| `remove_pose_strips` | Remove a character's held poses |
| `apply_pose` | Write a pose as bone keys at a time (pose-to-pose) |
| `save_pose` | Save a pose in the project, from joint values or a character at a time |
| `make_clip_from_poses` | Chain poses into a reusable animation, optionally given to a character |
| `command_reference` | This document |

To design a character from a description: `search_parts` for each idea, `create_character` with
a recipe, `preview_character` to look, then `update_character` to adjust. Recipes and the part
pack format are in [CHARACTERS.md](CHARACTERS.md).

A good loop: `get_library` → `script_to_video` or `apply_commands` → `render_frame` to check →
adjust → `export_video`.

## Screenplay format (`script_to_video`)

```text
Title: Coffee break
Style: stick                    optional: cast stick figures (women in dresses, men) instead of cartoon characters
[office]                        new scene (background id; optional length: [office, 6s])
Maya (happy, wave): Hi Leo!     line; words in brackets = expression and/or action
Leo (surprised): Oh, hi Maya!
> Meanwhile, at the park...     caption for this scene
[park]
Leo (walk): Nice day for a walk.
```

Expressions: neutral, happy, sad, angry, surprised, thinking, wink, worried, confused.
Actions: any action id from `get_library` (idle, talk, wave, walk, run, jump, cheer, think, point, shrug, clap, dance, sad,
worry, hips, meditate, slump, kick, confused, thumbs-up).
Stick backgrounds: stick-paper, stick-whiteboard, stick-hills, stick-room, stick-stage. Stick props are the `stick-*` ids.

## Commands (`apply_commands`)

All times are **frames** (project fps, default 30). Positions are stage pixels on a
1280×720 stage; characters and props are anchored at their feet/base, text at its centre.

```jsonc
{"op":"setMeta","name":"My film","fps":30,"width":1280,"height":720}
{"op":"addScene","scene":{"id":"s2","name":"Park","duration":150,"background":{"preset":"park"}},"index":1}
{"op":"updateScene","scene":{...full scene settings, items/audio are kept...}}
{"op":"removeScene","id":"s2"}            {"op":"moveScene","id":"s2","to":0}
{"op":"addItem","scene":"s1","item":{ITEM}} {"op":"updateItem","scene":"s1","item":{ITEM}}
{"op":"removeItem","scene":"s1","id":"maya"} {"op":"moveItem","scene":"s1","id":"maya","to":0}   // z-order, 0 = back
{"op":"addAudio","scene":"s1","clip":{"id":"c1","media":"m1","start":30,"speaker":"maya","volume":1}}
{"op":"updateAudio",...} {"op":"removeAudio","scene":"s1","id":"c1"}
{"op":"setKeys","scene":"s1","item":"maya","path":"transform.x","keys":[{"frame":0,"value":300,"interp":"easeInOut"},{"frame":60,"value":900}],"replace":false}
{"op":"deleteKeys","scene":"s1","item":"maya","path":"transform.x","frames":[60]}   // or "from"/"to"; no path = all tracks
{"op":"setInterpolation","scene":"s1","item":"maya","path":"transform.x","frames":null,"interp":"bounce"}
{"op":"defineOutfit","scene":"s1","item":"leo","name":"suit","outfit":{"top":"#222222","accessory":"tie"}}   // outfit null removes
{"op":"batch","commands":[...]}
```

Scene fields: `id, name, duration, background {preset, colors[]}, camera {from {x,y,zoom}, to {x,y,zoom}, moving, ease}, transition {kind: cut|fade|fadeBlack|slide|zoom, duration}`.

ITEM fields (all optional except `id` and `kind`):
`id, name, start, end (null = until scene end), x, y, scale, rotation, flip, opacity,
enter/exit (none|fade|pop|slideLeft|slideRight|slideUp), motion {x, y, start, end, ease}, locked, kind`.

`kind` is one of:

```jsonc
{"type":"character","action":"wave","expression":"happy","speed":1,"autoWalk":true,"autoTalk":true,
 "voice":"supertonic:F1","voiceSpeed":1,
 "look":{"skin":"#c68b62","hair":"#231a16","hairStyle":"short|spiky|long|bun|curly|bald",
         "top":"#f26b5b","bottom":"#2f3a56","shoes":"#2a221e","build":"slim|regular|broad",
         "accessory":"none|glasses|tie|cap|bow","accent":"#e84a5f",
         "style":"cartoon|stick","dress":false}}
{"type":"prop","prop":"desk","colors":["#b9875a"]}
{"type":"text","text":"Hello!","style":"bubble|thought|caption|title|plain","size":28,
 "color":"#1f2233","fill":"#ffffff","tail":"bottomLeft|bottomRight|none","width":320,"typewriter":0}
```

Easing: `linear | inOut | in | out | back`. Colours: `#rrggbb` or `#rrggbbaa`.
Characters with `motion` walk automatically and face where they're going; characters with a
`speaker` line lip-sync and gesture.

## Keyframes (`set_keys`, `setKeys`)

One keyframe engine animates everything. A **track** is a list of keys on one property path;
items and scenes carry a `tracks` map (`{"transform.x": {"keys": [...]}}`). Keyed values override
the item's own fields (and its `motion`), so unkeyed items behave exactly as before.

A key is `{"frame": 30, "value": ..., "interp": "easeInOut"}`. Frames are scene-local; the MCP
tools also accept `"seconds"`. `interp` shapes the segment from this key **to the next one**:
`constant` (hold, then jump), `linear` (default), `easeIn`, `easeOut`, `easeInOut`, `back`
(overshoot), `elastic`, `bounce`, or `{"bezier":[x1,y1,x2,y2]}` (CSS `cubic-bezier`). Before the
first key and after the last, a track holds that key's value. Numbers and colours blend; bools,
enums, expressions and outfits switch exactly on their key's frame.

| path | applies to | value |
|---|---|---|
| `transform.x`, `transform.y` | any item | stage px (characters walk automatically while x changes) |
| `transform.rotation` | any item | degrees, clockwise |
| `transform.scale` | any item | replaces the item's scale |
| `transform.scaleX`, `transform.scaleY` | any item | multipliers for squash and stretch |
| `transform.opacity` | any item | 0..1 |
| `transform.flip` | any item | bool |
| `bone.<bone>.rot` | character | degrees; overrides the current action while keyed |
| `look.skin`, `look.hair`, `look.top`, `look.bottom`, `look.shoes`, `look.accent` | character | `#rrggbb` |
| `look.hairStyle`, `look.build`, `look.accessory`, `look.style`, `look.dress` | character | enum name / bool |
| `outfit` | character | an outfit name, `"default"`, or a partial look object |
| `expression` | character | expression name |
| `camera.x`, `camera.y`, `camera.zoom` | scene (omit `item`) | stage px / zoom factor |

Bones: `bone.root.y` (lift, px), `bone.spine.y` (bob, px), `bone.spine.rot` (lean),
`bone.head.rot`, `bone.upper_arm_l.rot`, `bone.forearm_l.rot`, `bone.upper_arm_r.rot`,
`bone.forearm_r.rot`, `bone.thigh_l.rot`, `bone.shin_l.rot`, `bone.thigh_r.rot`, `bone.shin_r.rot`,
`bone.eyes.x` (-1..1), `bone.hand_r.thumb` (0..1). `_l`/`_r` are screen-left/right. Arm and leg angles
are measured from hanging straight down; positive raises the limb outward on its own side. Short
aliases (`bone.ls`, `bone.le`, `x`, `skin`, `costume`, ...) are accepted and stored under the names above.

Costume swap: `define_outfit` with `{"top":"#222222","bottom":"#222222","accessory":"tie"}`, then
`set_keys` path `outfit` with `[{"seconds":0,"value":"default"},{"seconds":2,"value":"suit"}]`.
Single `look.*` keys win over the outfit, so a character can turn green while wearing the suit.
