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
| `command_reference` | This document |

A good loop: `get_library` → `script_to_video` or `apply_commands` → `render_frame` to check →
adjust → `export_video`.

## Screenplay format (`script_to_video`)

```text
Title: Coffee break
[office]                        new scene (background id; optional length: [office, 6s])
Maya (happy, wave): Hi Leo!     line; words in brackets = expression and/or action
Leo (surprised): Oh, hi Maya!
> Meanwhile, at the park...     caption for this scene
[park]
Leo (walk): Nice day for a walk.
```

Expressions: neutral, happy, sad, angry, surprised, thinking, wink.
Actions: any action id from `get_library` (idle, talk, wave, walk, run, jump, cheer, think, point, shrug, clap, dance, sad).

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
{"op":"batch","commands":[...]}
```

Scene fields: `id, name, duration, background {preset, colors[]}, camera {from {x,y,zoom}, to {x,y,zoom}, moving, ease}, transition {kind: cut|fade|fadeBlack|slide|zoom, duration}`.

ITEM fields (all optional except `id` and `kind`):
`id, name, start, end (null = until scene end), x, y, scale, rotation, flip, opacity,
enter/exit (none|fade|pop|slideLeft|slideRight|slideUp), motion {x, y, start, end, ease}, locked, kind`.

`kind` is one of:

```jsonc
{"type":"character","action":"wave","expression":"happy","speed":1,"autoWalk":true,"autoTalk":true,
 "voice":"piper:en_US-ljspeech-medium","voiceSpeed":1,
 "look":{"skin":"#c68b62","hair":"#231a16","hairStyle":"short|spiky|long|bun|curly|bald",
         "top":"#f26b5b","bottom":"#2f3a56","shoes":"#2a221e","build":"slim|regular|broad",
         "accessory":"none|glasses|tie|cap|bow","accent":"#e84a5f"}}
{"type":"prop","prop":"desk","colors":["#b9875a"]}
{"type":"text","text":"Hello!","style":"bubble|thought|caption|title|plain","size":28,
 "color":"#1f2233","fill":"#ffffff","tail":"bottomLeft|bottomRight|none","width":320,"typewriter":0}
```

Easing: `linear | inOut | in | out | back`. Colours: `#rrggbb` or `#rrggbbaa`.
Characters with `motion` walk automatically and face where they're going; characters with a
`speaker` line lip-sync and gesture.
