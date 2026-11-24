//! MCP server (stdio, JSON-RPC 2.0, one message per line).
//!
//! Exposes the project, library, voices, the command API, frame previews and
//! export to AI agents. The project file is saved after every change, so the
//! desktop app (which watches the file) shows the agent's edits live.

use base64::Engine as _;
use frameling_core::*;
use frameling_tts::Tts;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

const REFERENCE: &str = include_str!("../../../docs/AGENTS.md");

pub struct Server {
    ed: Editor,
    lib: Library,
    tts: Tts,
    path: PathBuf,
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": schema })
}

fn tools() -> Value {
    let obj = |props: Value, required: &[&str]| json!({ "type": "object", "properties": props, "required": required });
    json!([
        tool("get_project", "Outline of the current project: scenes (ids, backgrounds, lengths), items (ids, names, positions, actions, voices) and spoken lines with timings. detail='full' returns the project JSON without audio data.", obj(json!({ "detail": { "type": "string", "enum": ["outline", "full"] } }), &[])),
        tool("get_library", "Available props, backgrounds, character actions, expressions and look options.", obj(json!({}), &[])),
        tool("list_voices", "Text-to-speech voices installed on this machine (ids to use in say_line or a character's voice). Built-in supertonic voices take an @<lang> suffix for other languages.", obj(json!({}), &[])),
        tool("apply_commands", "Apply editor commands as one undoable step. Call command_reference for the format. Example: [{\"op\":\"addItem\",\"scene\":\"scene-1\",\"item\":{\"id\":\"desk1\",\"x\":900,\"y\":640,\"kind\":{\"type\":\"prop\",\"prop\":\"desk\"}}}]", obj(json!({ "commands": { "type": "array", "items": { "type": "object" } } }), &["commands"])),
        tool("say_line", "Make a character speak: synthesises the text with a voice, lip-syncs the character, places the line after the previous one and lengthens the scene to fit.", obj(json!({
            "scene": { "type": "string", "description": "scene id" },
            "character": { "type": "string", "description": "character item id" },
            "text": { "type": "string" },
            "voice": { "type": "string", "description": "voice id; defaults to the character's voice" },
            "speed": { "type": "number", "description": "0.5-2, default 1" },
            "start_seconds": { "type": "number", "description": "optional fixed start time in the scene" }
        }), &["scene", "character", "text"])),
        tool("script_to_video", "Replace the project with one built from a screenplay: '[office]' starts a scene, 'Name (happy, wave): line' is dialogue, '> text' is a caption. Characters are cast and voiced automatically. A line 'Style: stick' casts stick figures instead of cartoon characters.", obj(json!({ "script": { "type": "string" }, "bubbles": { "type": "boolean", "description": "also show lines in speech bubbles" } }), &["script"])),
        tool("render_frame", "Render a frame as a PNG so you can see the result. Give scene + seconds (within that scene), or a global frame number.", obj(json!({
            "scene": { "type": "string" }, "seconds": { "type": "number" }, "frame": { "type": "integer" }, "width": { "type": "integer", "description": "default 640" }
        }), &[])),
        tool("export_video", "Export the whole project to a video file (.mp4 or .webm). Needs FFmpeg.", obj(json!({ "path": { "type": "string" }, "width": { "type": "integer" } }), &["path"])),
        tool("undo", "Undo the last change.", obj(json!({}), &[])),
        tool("redo", "Redo the last undone change.", obj(json!({}), &[])),
        tool("new_project", "Start a new project, blank or from the demo template.", obj(json!({ "template": { "type": "string", "enum": ["blank", "demo"] } }), &[])),
        tool("set_keys", "Keyframe any property over time: position, rotation, scale, squash/stretch, opacity, flip, bones (character joints), skin and clothing colours, outfit (costume) swaps, expression, or the scene camera. Keys at an existing frame are replaced. Each key's interp shapes the move to the NEXT key. Example: {\"scene\":\"s1\",\"item\":\"maya\",\"path\":\"transform.x\",\"keys\":[{\"seconds\":0,\"value\":300,\"interp\":\"easeInOut\"},{\"seconds\":2,\"value\":900}]}. Call list_tracks for every valid path.", obj(json!({
            "scene": { "type": "string" },
            "item": { "type": "string", "description": "item id; omit for scene tracks (camera.x, camera.y, camera.zoom)" },
            "path": { "type": "string", "description": "e.g. transform.x, transform.rotation, bone.forearm_r.rot, look.skin, outfit, expression, camera.zoom" },
            "keys": { "type": "array", "items": { "type": "object", "properties": {
                "frame": { "type": "integer", "description": "scene-local frame (or give seconds)" },
                "seconds": { "type": "number", "description": "time in the scene, converted to the nearest frame" },
                "value": { "description": "number, \"#rrggbb\" colour, bool, enum name, outfit name, or a partial look object for outfit" },
                "interp": { "description": "constant | linear (default) | easeIn | easeOut | easeInOut | back | elastic | bounce | {\"bezier\":[x1,y1,x2,y2]}" }
            }, "required": ["value"] } },
            "replace": { "type": "boolean", "description": "drop the track's other keys first" }
        }), &["scene", "path", "keys"])),
        tool("delete_keys", "Delete keys. Give frames or seconds to delete those keys, from/to (seconds, inclusive) for a range, or neither to delete the whole track. Omit path to clear every track on the item (or scene).", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" }, "path": { "type": "string" },
            "frames": { "type": "array", "items": { "type": "integer" } },
            "seconds": { "type": "array", "items": { "type": "number" } },
            "from_seconds": { "type": "number" }, "to_seconds": { "type": "number" }
        }), &["scene"])),
        tool("list_tracks", "List the keyframe tracks on an item (or on the scene if item is omitted), with every key's frame, time, value and interpolation, plus the item's outfits and the full path grammar.", obj(json!({ "scene": { "type": "string" }, "item": { "type": "string" } }), &["scene"])),
        tool("set_interpolation", "Change how keys ease into the next key (all keys on the track, or only those at the given frames/seconds).", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" }, "path": { "type": "string" },
            "interp": { "description": "constant | linear | easeIn | easeOut | easeInOut | back | elastic | bounce | {\"bezier\":[x1,y1,x2,y2]}" },
            "frames": { "type": "array", "items": { "type": "integer" } },
            "seconds": { "type": "array", "items": { "type": "number" } }
        }), &["scene", "path", "interp"])),
        tool("define_outfit", "Define a named costume on a character (a partial look: skin, hair, hairStyle, top, bottom, shoes, build, accessory, accent, style, dress). Switch to it with set_keys path 'outfit', value '<name>' (or 'default' for the character's own look). Pass outfit null to remove it.", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" }, "name": { "type": "string" },
            "outfit": { "type": ["object", "null"], "description": "e.g. {\"top\":\"#222222\",\"bottom\":\"#222222\",\"accessory\":\"tie\"}" }
        }), &["scene", "item", "name", "outfit"])),
        tool("list_part_categories", "Character creator categories (body, hair, face, clothes, wearables), how many parts each has, and the colour fields (skin, hair, top, bottom, shoes, accent). Start here when making a character.", obj(json!({}), &[])),
        tool("search_parts", "Find character parts by words matched against names and tags, e.g. 'chef moustache', 'winter', 'podcast'. Returns part ids to use in a recipe. Built-in ids look like 'hair:curly' or 'eyes:sleepy'; wearables from part packs have plain ids like 'chef-hat'.", obj(json!({
            "query": { "type": "string", "description": "words; empty lists everything" },
            "category": { "type": "string", "description": "limit to one category, e.g. hair, eyes, brows, nose, facialHair, body, outfit, headwear, eyewear, neckwear, earrings, back, held" },
            "style": { "type": "string", "enum": ["cartoon", "stick"], "description": "only parts that fit this figure style" },
            "limit": { "type": "integer", "description": "default 40" }
        }), &[])),
        tool("create_character", "Add a character built from a recipe: {\"style\":\"cartoon\"|\"stick\", \"parts\":[\"hair:curly\",\"facialHair:moustache\",{\"part\":\"chef-hat\",\"colors\":[\"#ffffff\"]}], \"colors\":{\"skin\":\"#8d5a3b\",\"hair\":\"#1b1311\",\"top\":\"#f1f1ef\",\"bottom\":\"#2f3a56\",\"shoes\":\"#2a221e\",\"accent\":\"#e84a5f\"}}. One part per category; parts you leave out keep the default. Check it with preview_character.", obj(json!({
            "scene": { "type": "string" }, "id": { "type": "string", "description": "new item id" }, "name": { "type": "string" },
            "recipe": { "type": "object" },
            "x": { "type": "number", "description": "default 640" }, "y": { "type": "number", "description": "feet, default 640" }, "scale": { "type": "number" },
            "action": { "type": "string" }, "expression": { "type": "string" }, "voice": { "type": "string" }
        }), &["scene", "id", "recipe"])),
        tool("update_character", "Change an existing character with a partial recipe: listed parts replace that category's part, listed colours change, everything else stays. Returns the full recipe.", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" }, "recipe": { "type": "object" }
        }), &["scene", "item", "recipe"])),
        tool("randomize_character", "Give a character (or a new one, for crowds and extras) a random look. Same seed = same result. keep lists categories or colour groups to leave alone (e.g. [\"hair\",\"skin\",\"clothes\"]); recipe pins parts first.", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string", "description": "existing character, or the id of a new one" },
            "seed": { "type": "integer" }, "style": { "type": "string", "enum": ["cartoon", "stick"] },
            "keep": { "type": "array", "items": { "type": "string" } }, "recipe": { "type": "object" },
            "x": { "type": "number" }, "y": { "type": "number" }
        }), &["scene", "item"])),
        tool("preview_character", "Render one character on its own as a PNG (optionally playing an action at a time) to check how a recipe looks.", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" }, "action": { "type": "string" }, "seconds": { "type": "number" }, "size": { "type": "integer", "description": "default 320" }
        }), &["scene", "item"])),
        tool("import_parts_pack", "Load a part pack (pack.json with a parts[] list) so its parts can be used. Characters that wear them carry copies inside the project file.", obj(json!({ "path": { "type": "string" } }), &["path"])),
        tool("list_poses", "The pose library: reusable poses (stand, sit, kneel, point, phone call, cheer, lying, walk stride...) that fit every character, plus poses saved in this project. Optional words filter by name and tags.", obj(json!({ "query": { "type": "string" }, "category": { "type": "string" } }), &[])),
        tool("add_pose_strip", "Hold a pose on a character for a while, blending in and out of whatever it was doing: e.g. sit from 2s to 6s. mask 'upper' poses only head and arms, so a character can point while walking. Strips stack in order.", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" }, "pose": { "type": "string", "description": "pose id from list_poses" },
            "start_seconds": { "type": "number" }, "end_seconds": { "type": "number" },
            "blend_seconds": { "type": "number", "description": "blend in and out, default 0.3" },
            "mask": { "type": "string", "enum": ["full", "upper", "lower", "armL", "armR", "head"] },
            "mirror": { "type": "boolean", "description": "use the left-right mirrored pose" },
            "breathe": { "type": "boolean", "description": "subtle breathing while held (default true)" }
        }), &["scene", "item", "pose", "start_seconds", "end_seconds"])),
        tool("remove_pose_strips", "Remove a character's pose strips: all of them, or those using one pose id.", obj(json!({ "scene": { "type": "string" }, "item": { "type": "string" }, "pose": { "type": "string" } }), &["scene", "item"])),
        tool("apply_pose", "Write a pose as bone keyframes at a time (pose-to-pose animation). Keys hold until the next key; the interp shapes the move to the next one. mask limits which joints are keyed.", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" }, "pose": { "type": "string" }, "seconds": { "type": "number" },
            "mask": { "type": "string", "enum": ["full", "upper", "lower", "armL", "armR", "head"] }, "mirror": { "type": "boolean" },
            "interp": { "description": "easeInOut (default), linear, back, bounce, ..." }
        }), &["scene", "item", "pose", "seconds"])),
        tool("save_pose", "Save a pose in the project: from a character as it is at a time (its action, strips and keys), or from explicit joint values ({ls, le, rs, re, lh, lk, rh, rk, lean, head, bob, rise, drop, fold, tilt, ground, look, thumb}).", obj(json!({
            "id": { "type": "string" }, "name": { "type": "string" }, "category": { "type": "string" }, "tags": { "type": "array", "items": { "type": "string" } },
            "scene": { "type": "string" }, "item": { "type": "string" }, "seconds": { "type": "number" }, "pose": { "type": "object" }
        }), &["id"])),
        tool("make_clip_from_poses", "Chain poses into a reusable animation (an action any character can play): hold each pose, then move to the next over gap_seconds. Optionally give it to a character right away.", obj(json!({
            "id": { "type": "string" }, "name": { "type": "string" },
            "poses": { "type": "array", "items": { "type": "object", "properties": { "pose": { "type": "string" }, "hold": { "type": "number", "description": "seconds" }, "mirror": { "type": "boolean" } }, "required": ["pose"] } },
            "gap_seconds": { "type": "number", "description": "default 0.4" }, "loop": { "type": "boolean" },
            "scene": { "type": "string" }, "item": { "type": "string", "description": "character to play it" }
        }), &["id", "poses"])),
        tool("turn_character", "Turn a character to one of 8 views: front, frontRight, right, backRight, back, backLeft, left, frontLeft (right/left = facing screen right/left; 3/4 views in between). Without seconds it sets how the character faces; with seconds it keys a turn at that time. auto_turn (default on) makes walking characters face the way they go.", obj(json!({
            "scene": { "type": "string" }, "item": { "type": "string" },
            "view": { "type": "string", "enum": ["front", "frontRight", "right", "backRight", "back", "backLeft", "left", "frontLeft"] },
            "seconds": { "type": "number" }, "auto_turn": { "type": "boolean" }
        }), &["scene", "item"])),
        tool("command_reference","Full reference for commands, item fields, the screenplay format and the workflow.", obj(json!({}), &[])),
    ])
}

/// The track path grammar, generated from the engine's own tables.
fn path_help() -> String {
    let mut o = String::from("\nPaths:\n  any item: transform.x, transform.y (scene px), transform.rotation (degrees clockwise), transform.scale, transform.scaleX / transform.scaleY (multipliers, squash and stretch), transform.opacity (0-1), transform.flip (bool)\n  characters: look.skin, look.hair, look.top, look.bottom, look.shoes, look.accent (\"#rrggbb\", blend smoothly); look.hairStyle, look.build, look.accessory, look.style, look.dress, outfit, expression (switch on the key's frame)\n  scene (omit item): camera.x, camera.y, camera.zoom\n  character bones (override the current action while keyed):\n");
    for (path, alias, _, what) in anim::path::BONES {
        o += &format!("    {path} (alias {alias}): {what}\n");
    }
    o += "Keyed values override the item's own fields. Before the first key and after the last, a track holds that key's value.\n";
    o
}

fn text(s: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": s.into() }] })
}

impl Server {
    pub fn new(path: PathBuf, lib: Library, tts: Tts) -> Result<Server, String> {
        let project = if path.exists() {
            Project::from_json(&std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?)?
        } else {
            demo::blank_project()
        };
        let s = Server { ed: Editor::new(project), lib, tts, path };
        s.save()?;
        Ok(s)
    }

    fn save(&self) -> Result<(), String> {
        let tmp = self.path.with_extension("frameling.tmp");
        std::fs::write(&tmp, self.ed.project.to_json_pretty()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    pub fn outline(&self) -> String {
        let p = &self.ed.project;
        let fps = p.fps as f32;
        let mut o = format!("Project {:?}: {}x{} @ {} fps, {} scenes, {:.1}s total\n", p.name, p.width, p.height, p.fps, p.scenes.len(), p.total_frames() as f32 / fps);
        for (i, s) in p.scenes.iter().enumerate() {
            o += &format!("\n[{}] scene id={:?} name={:?} background={} length={:.1}s transition={:?}\n", i + 1, s.id, s.name, s.background.preset, s.duration as f32 / fps, s.transition.kind);
            if !s.tracks.is_empty() {
                o += &format!("  camera keyed: {}\n", s.tracks.iter().map(|(p, t)| format!("{p}({})", t.keys.len())).collect::<Vec<_>>().join(", "));
            }
            for it in &s.items {
                let what = match &it.kind {
                    ItemKind::Character(c) => format!("character action={} expression={:?} voice={}", c.action, c.expression, c.voice.as_deref().unwrap_or("-")),
                    ItemKind::Prop(pr) => format!("prop {}", pr.prop),
                    ItemKind::Text(t) => format!("text {:?} {:?}", t.style, t.text),
                    ItemKind::Image(im) => format!("image {} width={}", im.media, im.width),
                };
                let timing = match it.end {
                    Some(e) => format!(" {:.1}-{:.1}s", it.start as f32 / fps, e as f32 / fps),
                    None if it.start > 0 => format!(" from {:.1}s", it.start as f32 / fps),
                    None => String::new(),
                };
                let mv = it.motion.map(|m| format!(" moves to ({:.0},{:.0}) {:.1}-{:.1}s", m.x, m.y, m.start as f32 / fps, m.end as f32 / fps)).unwrap_or_default();
                let keyed = if it.tracks.is_empty() {
                    String::new()
                } else {
                    format!(" keyed: {}", it.tracks.iter().map(|(p, t)| format!("{p}({})", t.keys.len())).collect::<Vec<_>>().join(", "))
                };
                o += &format!("  - id={:?} {:?} at ({:.0},{:.0}) scale {:.2}{}{}: {}{}\n", it.id, it.name, it.x, it.y, it.scale, timing, mv, what, keyed);
            }
            for c in &s.audio {
                let m = p.media.get(&c.media);
                let what = match m.and_then(|m| m.source.as_ref()) {
                    Some(MediaSource::Tts { text, voice, .. }) => format!("{text:?} ({voice})"),
                    _ => m.map(|m| m.name.clone()).unwrap_or_default(),
                };
                let len = m.map(|m| c.play_frames(m)).unwrap_or(0) as f32 / fps;
                o += &format!("  ♪ clip id={:?} {:.1}-{:.1}s speaker={}: {}\n", c.id, c.start as f32 / fps, c.start as f32 / fps + len, c.speaker.as_deref().unwrap_or("-"), what);
            }
        }
        o
    }

    /// A frame from `frame_key` (integer) or `secs_key` (seconds, rounded).
    fn frame_arg(&self, v: &Value, frame_key: &str, secs_key: &str) -> Result<Option<u32>, String> {
        if let Some(f) = v.get(frame_key).filter(|f| !f.is_null()) {
            return f.as_u64().map(|f| Some(f as u32)).ok_or_else(|| format!("{frame_key} must be a whole number >= 0"));
        }
        match v.get(secs_key).filter(|f| !f.is_null()) {
            Some(t) => {
                let t = t.as_f64().ok_or_else(|| format!("{secs_key} must be a number"))?;
                if t < 0.0 {
                    return Err(format!("{secs_key} must be >= 0"));
                }
                Ok(Some((t * self.ed.project.fps as f64).round() as u32))
            }
            None => Ok(None),
        }
    }

    /// `frames` and/or `seconds` arrays merged into one frame list.
    fn frames_arg(&self, a: &Value) -> Result<Option<Vec<u32>>, String> {
        let fps = self.ed.project.fps as f64;
        let mut out = Vec::new();
        let mut any = false;
        if let Some(f) = a.get("frames").and_then(Value::as_array) {
            any = true;
            out.extend(f.iter().map(|v| v.as_u64().map(|v| v as u32).ok_or("frames must be whole numbers")).collect::<Result<Vec<_>, _>>()?);
        }
        if let Some(f) = a.get("seconds").and_then(Value::as_array) {
            any = true;
            out.extend(f.iter().map(|v| v.as_f64().map(|t| (t * fps).round().max(0.0) as u32).ok_or("seconds must be numbers")).collect::<Result<Vec<_>, _>>()?);
        }
        Ok(any.then_some(out))
    }

    /// Human-readable tracks of an item (or the scene), with times.
    fn tracks_text(&self, scene: &str, item: Option<&str>) -> Result<String, String> {
        let p = &self.ed.project;
        let fps = p.fps as f32;
        let sc = p.scene(scene).ok_or_else(|| format!("no scene {scene:?}"))?;
        let (label, tracks, outfits) = match item {
            Some(id) => {
                let it = sc.item(id).ok_or_else(|| format!("no item {id:?} in scene {scene:?}"))?;
                let outfits = match &it.kind {
                    ItemKind::Character(c) => Some(&c.outfits),
                    _ => None,
                };
                (format!("item {id:?}"), &it.tracks, outfits)
            }
            None => (format!("scene {scene:?}"), &sc.tracks, None),
        };
        let mut o = if tracks.is_empty() { format!("No tracks on {label}.\n") } else { format!("Tracks on {label}:\n") };
        for (path, t) in tracks {
            o += &format!("  {path}:\n");
            for k in &t.keys {
                let interp = serde_json::to_value(k.interp).map(|v| v.to_string()).unwrap_or_default();
                o += &format!("    frame {} ({:.2}s) = {}  {}\n", k.frame, k.frame as f32 / fps, k.value, interp);
            }
        }
        if let Some(of) = outfits.filter(|o| !o.is_empty()) {
            o += &format!("Outfits: {}\n", serde_json::to_string(of).unwrap_or_default());
        }
        Ok(o)
    }

    /// Add or replace a character, copy in any pack parts it wears, save, and
    /// report its recipe.
    fn put_character(&mut self, scene: &str, item: Item, create: bool) -> Result<Value, String> {
        let id = item.id.clone();
        let ItemKind::Character(c) = &item.kind else { return Err("not a character".into()) };
        let builtin = &Library::builtin().parts;
        let defs: Vec<_> = c.look.wear.iter().filter(|w| !builtin.contains_key(&w.part) && !self.ed.project.parts.contains_key(&w.part)).filter_map(|w| self.lib.parts.get(&w.part).map(|p| p.def.clone())).collect();
        let recipe = parts::Recipe::from_look(&c.look, &self.lib, Some(&self.ed.project));
        let mut cmds = vec![];
        if !defs.is_empty() {
            cmds.push(Command::AddParts { parts: defs });
        }
        cmds.push(if create { Command::AddItem { scene: scene.into(), item, index: None } } else { Command::UpdateItem { scene: scene.into(), item } });
        self.ed.apply(&Command::Batch { commands: cmds }, None)?;
        self.save()?;
        Ok(text(format!("{} {id}. Recipe: {}\nLook at it with preview_character.", if create { "Created" } else { "Updated" }, serde_json::to_string(&recipe).unwrap())))
    }

    fn call(&mut self, name: &str, a: &Value) -> Result<Value, String> {
        let s = |k: &str| a.get(k).and_then(Value::as_str);
        match name {
            "get_project" => {
                if s("detail") == Some("full") {
                    let mut p = self.ed.project.clone();
                    for m in p.media.values_mut() {
                        let mut lite = (**m).clone();
                        lite.data = String::new();
                        lite.envelope.clear();
                        *m = std::sync::Arc::new(lite);
                    }
                    Ok(text(p.to_json_pretty()))
                } else {
                    Ok(text(self.outline()))
                }
            }
            "get_library" => {
                let mut c = self.lib.catalog();
                c["expressions"] = json!(["neutral", "happy", "sad", "angry", "surprised", "thinking", "wink", "worried", "confused"]);
                c["styles"] = json!({ "style": ["cartoon", "stick"], "dress": "true/false (stick figures)", "stickTip": "a stick figure is look.style='stick': lines use look.top, head fill look.skin, dress/bow/cap/tie use look.accent" });
                c["look"] = json!({ "hairStyle": ["short", "spiky", "long", "bun", "curly", "bald"], "build": ["slim", "regular", "broad"], "accessory": ["none", "glasses", "tie", "cap", "bow"] }); c["views"] = json!(["front", "frontRight", "right", "backRight", "back", "backLeft", "left", "frontLeft"]);
                c["textStyles"] = json!(["bubble", "thought", "caption", "title", "plain"]);
                Ok(text(serde_json::to_string_pretty(&c).unwrap()))
            }
            "list_voices" => {
                let v = self.tts.voices();
                let mut out = if v.is_empty() { "No voices installed.".to_string() } else { serde_json::to_string_pretty(&v).unwrap() };
                if self.tts.model_dir.is_some() {
                    out.push_str("

supertonic voices speak 31 languages (en ko ja ar bg cs da de el es et fi fr hi hr hu id it lt lv nl pl pt ro ru sk sl sv tr uk vi): append @<lang> to the id, e.g. supertonic:M2@es. Inline <laugh>, <breath> and <sigh> tags work in lines.

Licence terms the user has agreed to (pass them on, and label published audio as machine-generated): ");
                    out.push_str(frameling_tts::SUPERTONIC_TERMS);
                }
                Ok(text(out))
            }
            "apply_commands" => {
                let cmds: Vec<Command> = serde_json::from_value(a.get("commands").cloned().unwrap_or(json!([]))).map_err(|e| format!("bad commands: {e}"))?;
                let n = cmds.len();
                self.ed.apply(&Command::Batch { commands: cmds }, None)?;
                self.save()?;
                Ok(text(format!("Applied {n} command(s).")))
            }
            "say_line" => {
                let (scene, who, line) = (s("scene").ok_or("scene is required")?, s("character").ok_or("character is required")?, s("text").ok_or("text is required")?);
                let item = self.ed.project.scene(scene).and_then(|sc| sc.item(who)).ok_or("no such character in that scene")?;
                let ItemKind::Character(c) = &item.kind else { return Err("that item is not a character".into()) };
                let voice = s("voice").map(String::from).or_else(|| c.voice.clone()).or_else(|| self.tts.voices().first().map(|v| v.id.clone())).ok_or("no voices installed")?;
                let speed = a.get("speed").and_then(Value::as_f64).map_or(c.voice_speed, |v| v as f32);
                let wav = self.tts.synthesize(line, &voice, speed)?;
                let fps = self.ed.project.fps;
                let (frames, envelope) = audio::analyse_wav(&wav, fps)?;
                let n = self.ed.project.media.len();
                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
                let media = Media {
                    id: format!("tts-{stamp}-{n}"),
                    name: format!("{}.wav", line.chars().take(24).collect::<String>()),
                    mime: "audio/wav".into(),
                    data: base64::engine::general_purpose::STANDARD.encode(&wav),
                    duration_frames: frames,
                    envelope,
                    source: Some(MediaSource::Tts { text: line.into(), voice: voice.clone(), speed }),
                    size: None,
                };
                let start = a.get("start_seconds").and_then(Value::as_f64).map(|t| (t * fps as f64).round() as u32);
                let clip_id = format!("line-{stamp}-{n}");
                self.ed.apply(&Command::AddLine { scene: scene.into(), clip_id: clip_id.clone(), media, speaker: Some(who.into()), start }, None)?;
                self.save()?;
                let at = self.ed.project.scene(scene).and_then(|sc| sc.audio.iter().find(|c| c.id == clip_id)).map_or(0, |c| c.start);
                Ok(text(format!("{who} says it at {:.1}s for {:.1}s (voice {voice}, clip id {clip_id}).", at as f32 / fps as f32, frames as f32 / fps as f32)))
            }
            "script_to_video" => {
                let script = s("script").ok_or("script is required")?;
                let bubbles = a.get("bubbles").and_then(Value::as_bool).unwrap_or(false);
                let (p, log) = crate::screenplay::build(script, &self.lib, &self.tts, &crate::screenplay::Options { bubbles })?;
                self.ed.apply(&Command::Batch { commands: vec![] }, None)?; // undo point
                self.ed.project = p;
                self.save()?;
                Ok(text(format!("{}\n\n{}", log.join("\n"), self.outline())))
            }
            "render_frame" => {
                let p = &self.ed.project;
                let frame = if let Some(id) = s("scene") {
                    let i = p.scene_index(id).ok_or("no such scene")?;
                    let local = (a.get("seconds").and_then(Value::as_f64).unwrap_or(0.0) * p.fps as f64).round() as u32;
                    p.scene_starts()[i] + local.min(p.scenes[i].duration.saturating_sub(1))
                } else {
                    a.get("frame").and_then(Value::as_u64).unwrap_or(0) as u32
                };
                let w = a.get("width").and_then(Value::as_u64).unwrap_or(640).clamp(64, 1920) as u32;
                let h = w * p.height / p.width;
                let png = frameling_render::render_png(&evaluate(p, &self.lib, frame), w, h)?;
                Ok(json!({ "content": [
                    { "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png" },
                    { "type": "text", "text": format!("frame {frame} of {}", p.total_frames()) }
                ]}))
            }
            "export_video" => {
                let out = PathBuf::from(s("path").ok_or("path is required")?);
                let p = &self.ed.project;
                let mut opts = frameling_export::ExportOptions::for_project(p);
                if let Some(w) = a.get("width").and_then(Value::as_u64) {
                    opts.width = w as u32;
                    opts.height = w as u32 * p.height / p.width;
                }
                frameling_export::export_video(p, &self.lib, &out, &opts, |_, _| true)?;
                Ok(text(format!("Exported {} ({}x{}, {:.1}s).", out.display(), opts.width, opts.height, p.total_frames() as f32 / p.fps as f32)))
            }
            "undo" => {
                let ok = self.ed.undo();
                self.save()?;
                Ok(text(if ok { "Undone." } else { "Nothing to undo." }))
            }
            "redo" => {
                let ok = self.ed.redo();
                self.save()?;
                Ok(text(if ok { "Redone." } else { "Nothing to redo." }))
            }
            "new_project" => {
                let p = if s("template") == Some("demo") { demo::demo_project() } else { demo::blank_project() };
                self.ed.apply(&Command::Batch { commands: vec![] }, None)?;
                self.ed.project = p;
                self.save()?;
                Ok(text(self.outline()))
            }
            "set_keys" => {
                let scene = s("scene").ok_or("scene is required")?;
                let keys = a.get("keys").and_then(Value::as_array).ok_or("keys must be an array")?;
                let keys = keys
                    .iter()
                    .map(|k| {
                        let frame = self.frame_arg(k, "frame", "seconds")?.ok_or("each key needs frame or seconds")?;
                        let value = k.get("value").cloned().ok_or("each key needs a value")?;
                        let interp = match k.get("interp") {
                            Some(i) => serde_json::from_value(i.clone()).map_err(|e| format!("bad interp {i}: {e}"))?,
                            None => Interp::Linear,
                        };
                        Ok(Key { frame, value, interp })
                    })
                    .collect::<Result<Vec<Key>, String>>()?;
                let path = s("path").ok_or("path is required")?;
                let n = keys.len();
                let replace = a.get("replace").and_then(Value::as_bool).unwrap_or(false);
                self.ed.apply(&Command::SetKeys { scene: scene.into(), item: s("item").map(Into::into), path: path.into(), keys, replace }, None)?;
                self.save()?;
                let canon = anim::Channel::parse(path)?.path();
                Ok(text(format!("Set {n} key(s) on {canon}.\n\n{}", self.tracks_text(scene, s("item"))?)))
            }
            "delete_keys" => {
                let scene = s("scene").ok_or("scene is required")?;
                let frames = self.frames_arg(a)?;
                let from = self.frame_arg(a, "from_frame", "from_seconds")?;
                let to = self.frame_arg(a, "to_frame", "to_seconds")?;
                self.ed.apply(&Command::DeleteKeys { scene: scene.into(), item: s("item").map(Into::into), path: s("path").map(Into::into), frames, from, to }, None)?;
                self.save()?;
                Ok(text(format!("Deleted.\n\n{}", self.tracks_text(scene, s("item"))?)))
            }
            "list_tracks" => Ok(text(self.tracks_text(s("scene").ok_or("scene is required")?, s("item"))? + &path_help())),
            "set_interpolation" => {
                let scene = s("scene").ok_or("scene is required")?;
                let interp: Interp = serde_json::from_value(a.get("interp").cloned().ok_or("interp is required")?).map_err(|e| format!("bad interp: {e}"))?;
                let frames = self.frames_arg(a)?;
                self.ed.apply(&Command::SetInterpolation { scene: scene.into(), item: s("item").map(Into::into), path: s("path").ok_or("path is required")?.into(), frames, interp }, None)?;
                self.save()?;
                Ok(text(format!("Done.\n\n{}", self.tracks_text(scene, s("item"))?)))
            }
            "define_outfit" => {
                let (scene, item, name) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?, s("name").ok_or("name is required")?);
                let outfit: Option<LookPatch> = match a.get("outfit") {
                    None | Some(Value::Null) => None,
                    Some(o) => Some(serde_json::from_value(o.clone()).map_err(|e| format!("bad outfit: {e}"))?),
                };
                let removing = outfit.is_none();
                self.ed.apply(&Command::DefineOutfit { scene: scene.into(), item: item.into(), name: name.into(), outfit }, None)?;
                self.save()?;
                Ok(text(if removing { format!("Removed outfit {name:?}.") } else { format!("Outfit {name:?} defined on {item}. Switch to it with set_keys path=outfit value={name:?}.") }))
            }
            "list_part_categories" => {
                let cat = parts::catalog(&self.lib, None);
                let mut o = String::from("Categories (one part each):\n");
                for (id, tab, label) in parts::CATEGORIES {
                    let n = cat.iter().filter(|p| p.category == *id).count();
                    o += &format!("  {id} ({tab} tab, {label}): {n} parts\n");
                }
                o += "Colour fields: skin, hair, top, bottom, shoes, accent (\"#rrggbb\"). Built-in parts that use a colour say so (colorField); a wearable's own colours go in its {part, colors} entry.\n";
                o += "Face parts (eyes, brows, nose) apply to cartoon figures; stick figures use their own simple face.\n";
                Ok(text(o))
            }
            "search_parts" => {
                let limit = a.get("limit").and_then(Value::as_u64).unwrap_or(40) as usize;
                let found = parts::search(&self.lib, s("query").unwrap_or(""), s("category"), s("style"));
                let n = found.len();
                let rows: Vec<Value> = found
                    .into_iter()
                    .take(limit)
                    .map(|p| json!({ "id": p.id, "name": p.name, "category": p.category, "tags": p.tags.join(" "), "styles": p.styles, "colors": p.colors, "colorField": p.color_field }))
                    .collect();
                Ok(text(format!("{n} part(s){}\n{}", if n > limit { format!(", first {limit}") } else { String::new() }, serde_json::to_string_pretty(&rows).unwrap())))
            }
            "create_character" => {
                let scene = s("scene").ok_or("scene is required")?;
                let id = s("id").ok_or("id is required")?;
                let recipe: parts::Recipe = serde_json::from_value(a.get("recipe").cloned().unwrap_or(json!({}))).map_err(|e| format!("bad recipe: {e}"))?;
                let look = recipe.to_look(None, &self.lib, Some(&self.ed.project))?;
                let num = |k: &str| a.get(k).and_then(Value::as_f64).map(|v| v as f32);
                let mut c = CharacterItem { look, ..Default::default() };
                if let Some(v) = s("action") {
                    c.action = v.into();
                }
                if let Some(v) = s("expression") {
                    c.expression = serde_json::from_value(json!(v)).map_err(|_| format!("unknown expression {v:?}"))?;
                }
                c.voice = s("voice").map(String::from);
                let item = Item {
                    id: id.into(),
                    name: s("name").unwrap_or(id).into(),
                    x: num("x").unwrap_or(640.0),
                    y: num("y").unwrap_or(640.0),
                    scale: num("scale").unwrap_or(0.95),
                    kind: ItemKind::Character(c),
                    ..Default::default()
                };
                self.put_character(scene, item, true)
            }
            "update_character" => {
                let (scene, id) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?);
                let recipe: parts::Recipe = serde_json::from_value(a.get("recipe").cloned().unwrap_or(json!({}))).map_err(|e| format!("bad recipe: {e}"))?;
                let mut item = self.ed.project.scene(scene).and_then(|sc| sc.item(id)).cloned().ok_or("no such character in that scene")?;
                let ItemKind::Character(c) = &mut item.kind else { return Err("that item is not a character".into()) };
                c.look = recipe.to_look(Some(&c.look), &self.lib, Some(&self.ed.project))?;
                self.put_character(scene, item, false)
            }
            "randomize_character" => {
                let (scene, id) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?);
                let seed = a.get("seed").and_then(Value::as_u64).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1));
                let keep: Vec<String> = serde_json::from_value(a.get("keep").cloned().unwrap_or(json!([]))).map_err(|e| format!("bad keep: {e}"))?;
                let existing = self.ed.project.scene(scene).and_then(|sc| sc.item(id)).cloned();
                let creating = existing.is_none();
                let mut item = existing.unwrap_or_else(|| Item {
                    id: id.into(),
                    name: id.into(),
                    x: a.get("x").and_then(Value::as_f64).unwrap_or(640.0) as f32,
                    y: a.get("y").and_then(Value::as_f64).unwrap_or(640.0) as f32,
                    scale: 0.95,
                    kind: ItemKind::Character(CharacterItem::default()),
                    ..Default::default()
                });
                let ItemKind::Character(c) = &mut item.kind else { return Err("that item is not a character".into()) };
                let mut base = c.look.clone();
                if let Some(st) = s("style") {
                    base = parts::Recipe { style: Some(st.into()), ..Default::default() }.to_look(Some(&base), &self.lib, Some(&self.ed.project))?;
                }
                let mut look = parts::randomize(&base, &self.lib, Some(&self.ed.project), seed, &keep);
                if let Some(r) = a.get("recipe") {
                    let r: parts::Recipe = serde_json::from_value(r.clone()).map_err(|e| format!("bad recipe: {e}"))?;
                    look = r.to_look(Some(&look), &self.lib, Some(&self.ed.project))?;
                }
                c.look = look;
                let mut v = self.put_character(scene, item, creating)?;
                if let Some(t) = v["content"][0]["text"].as_str() {
                    v["content"][0]["text"] = json!(format!("seed {seed}. {t}"));
                }
                Ok(v)
            }
            "preview_character" => {
                let (scene, id) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?);
                let p = &self.ed.project;
                let mut item = p.scene(scene).and_then(|sc| sc.item(id)).cloned().ok_or("no such item in that scene")?;
                if let (ItemKind::Character(c), Some(act)) = (&mut item.kind, s("action")) {
                    c.action = act.into();
                }
                let size = a.get("size").and_then(Value::as_u64).unwrap_or(320).clamp(64, 1024) as u32;
                let frame = (a.get("seconds").and_then(Value::as_f64).unwrap_or(0.3) * p.fps as f64).round() as u32;
                let dl = eval::item_preview(&self.lib, &p.parts, &item, size, size, frame);
                let png = frameling_render::render_png(&dl, size, size)?;
                Ok(json!({ "content": [
                    { "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png" },
                    { "type": "text", "text": format!("{id}") }
                ]}))
            }
            "import_parts_pack" => {
                let path = s("path").ok_or("path is required")?;
                let json = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                let pack: frameling_core::pack::PackFile = serde_json::from_str(&json).map_err(|e| format!("invalid pack: {e}"))?;
                let ids: Vec<String> = pack.parts.iter().map(|p| p.id.clone()).collect();
                let pid = self.lib.add_pack(pack)?;
                Ok(text(format!("Loaded pack {pid:?} with {} part(s): {}", ids.len(), ids.join(", "))))
            }
            "list_poses" => {
                let q = s("query").unwrap_or("").to_lowercase();
                let words: Vec<&str> = q.split_whitespace().collect();
                let mut all: Vec<&frameling_core::pack::PoseDef> = self.ed.project.poses.values().collect();
                all.extend(self.lib.poses.values().filter(|p| !self.ed.project.poses.contains_key(&p.id)));
                let rows: Vec<String> = all
                    .into_iter()
                    .filter(|p| s("category").is_none_or(|c| p.category.eq_ignore_ascii_case(c)))
                    .filter(|p| {
                        let hay = format!("{} {} {}", p.id, p.name, p.tags.join(" ")).to_lowercase();
                        words.iter().all(|w| hay.contains(w))
                    })
                    .map(|p| format!("  {} ({}) [{}]: {}", p.id, p.name, p.category, p.tags.join(" ")))
                    .collect();
                Ok(text(format!("{} pose(s):\n{}\nUse them with add_pose_strip (hold, blends, masks), apply_pose (keys) or make_clip_from_poses.", rows.len(), rows.join("\n"))))
            }
            "add_pose_strip" => {
                let (scene, id, pose) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?, s("pose").ok_or("pose is required")?);
                if poses::find(Some(&self.ed.project), &self.lib, pose).is_none() {
                    return Err(format!("unknown pose {pose:?}; list_poses shows the ids"));
                }
                let fps = self.ed.project.fps as f64;
                let num = |k: &str| a.get(k).and_then(Value::as_f64);
                let start = (num("start_seconds").unwrap_or(0.0) * fps).round().max(0.0) as u32;
                let end = (num("end_seconds").unwrap_or(0.0) * fps).round().max(0.0) as u32;
                if end <= start {
                    return Err("end_seconds must be after start_seconds".into());
                }
                let blend = (num("blend_seconds").unwrap_or(0.3) * fps).round().max(0.0) as u32;
                let mask: poses::PoseMask = match s("mask") {
                    Some(m) => serde_json::from_value(json!(m)).map_err(|_| format!("unknown mask {m:?}"))?,
                    None => poses::PoseMask::Full,
                };
                let strip = poses::PoseStrip {
                    pose: pose.into(),
                    start,
                    end,
                    blend_in: blend.min((end - start) / 2),
                    blend_out: blend.min((end - start) / 2),
                    mask,
                    mirror: a.get("mirror").and_then(Value::as_bool).unwrap_or(false),
                    breathe: a.get("breathe").and_then(Value::as_bool).unwrap_or(true),
                    custom: None,
                };
                let mut item = self.ed.project.scene(scene).and_then(|sc| sc.item(id)).cloned().ok_or("no such character in that scene")?;
                let ItemKind::Character(c) = &mut item.kind else { return Err("that item is not a character".into()) };
                c.poses.push(strip);
                let n = c.poses.len();
                self.ed.apply(&Command::UpdateItem { scene: scene.into(), item }, None)?;
                self.save()?;
                Ok(text(format!("{id} holds {pose:?} from {:.2}s to {:.2}s ({n} strip(s) on it). Check with render_frame.", start as f64 / fps, end as f64 / fps)))
            }
            "remove_pose_strips" => {
                let (scene, id) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?);
                let mut item = self.ed.project.scene(scene).and_then(|sc| sc.item(id)).cloned().ok_or("no such character in that scene")?;
                let ItemKind::Character(c) = &mut item.kind else { return Err("that item is not a character".into()) };
                let before = c.poses.len();
                match s("pose") {
                    Some(p) => c.poses.retain(|x| x.pose != p),
                    None => c.poses.clear(),
                }
                let removed = before - c.poses.len();
                self.ed.apply(&Command::UpdateItem { scene: scene.into(), item }, None)?;
                self.save()?;
                Ok(text(format!("Removed {removed} pose strip(s).")))
            }
            "apply_pose" => {
                let (scene, id, pose) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?, s("pose").ok_or("pose is required")?);
                let def = poses::find(Some(&self.ed.project), &self.lib, pose).ok_or_else(|| format!("unknown pose {pose:?}; list_poses shows the ids"))?;
                let p = if a.get("mirror").and_then(Value::as_bool).unwrap_or(false) { poses::mirror(&def.pose) } else { def.pose };
                let mask: poses::PoseMask = match s("mask") {
                    Some(m) => serde_json::from_value(json!(m)).map_err(|_| format!("unknown mask {m:?}"))?,
                    None => poses::PoseMask::Full,
                };
                let interp: Interp = match a.get("interp") {
                    Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("bad interp: {e}"))?,
                    None => Interp::EaseInOut,
                };
                let frame = (a.get("seconds").and_then(Value::as_f64).unwrap_or(0.0) * self.ed.project.fps as f64).round().max(0.0) as u32;
                let cmds: Vec<Command> = poses::pose_keys(&p, mask)
                    .into_iter()
                    .map(|(path, v)| Command::SetKeys { scene: scene.into(), item: Some(id.into()), path: path.into(), keys: vec![Key::new(frame, v).with(interp)], replace: false })
                    .collect();
                let n = cmds.len();
                self.ed.apply(&Command::Batch { commands: cmds }, None)?;
                self.save()?;
                Ok(text(format!("Keyed {pose:?} on {id} at frame {frame} ({n} joints). Keys hold until the next key; add another pose later to animate between them.")))
            }
            "save_pose" => {
                let pid = s("id").ok_or("id is required")?;
                let pose: frameling_core::rig::Pose = if let Some(v) = a.get("pose") {
                    serde_json::from_value(v.clone()).map_err(|e| format!("bad pose: {e}"))?
                } else {
                    let (scene, id) = (s("scene").ok_or("give pose values, or scene + item (+ seconds)")?, s("item").ok_or("item is required")?);
                    let local = (a.get("seconds").and_then(Value::as_f64).unwrap_or(0.0) * self.ed.project.fps as f64).round().max(0.0) as u32;
                    eval::posed_character(&self.ed.project, &self.lib, scene, id, local).ok_or("no such character in that scene")?
                };
                let tags: Vec<String> = serde_json::from_value(a.get("tags").cloned().unwrap_or(json!([]))).unwrap_or_default();
                let def = frameling_core::pack::PoseDef { id: pid.into(), name: s("name").unwrap_or(pid).into(), category: s("category").unwrap_or("Saved").into(), tags, pose };
                self.ed.apply(&Command::AddPose { pose: def }, None)?;
                self.save()?;
                Ok(text(format!("Saved pose {pid:?} in the project.")))
            }
            "make_clip_from_poses" => {
                let aid = s("id").ok_or("id is required")?;
                let steps: Vec<poses::ChainStep> = serde_json::from_value(a.get("poses").cloned().unwrap_or(json!([]))).map_err(|e| format!("bad poses: {e}"))?;
                let gap = a.get("gap_seconds").and_then(Value::as_f64).unwrap_or(0.4) as f32;
                let looped = a.get("loop").and_then(Value::as_bool).unwrap_or(false);
                let action = poses::chain(aid, s("name").unwrap_or(aid), &steps, gap, looped, Some(&self.ed.project), &self.lib)?;
                let dur = action.duration;
                let mut cmds = vec![Command::AddAction { action }];
                if let (Some(scene), Some(id)) = (s("scene"), s("item")) {
                    let mut item = self.ed.project.scene(scene).and_then(|sc| sc.item(id)).cloned().ok_or("no such character in that scene")?;
                    let ItemKind::Character(c) = &mut item.kind else { return Err("that item is not a character".into()) };
                    c.action = aid.into();
                    cmds.push(Command::UpdateItem { scene: scene.into(), item });
                }
                self.ed.apply(&Command::Batch { commands: cmds }, None)?;
                self.save()?;
                Ok(text(format!("Made animation {aid:?} ({:.2}s{}). Any character can play it as its action.", dur, if looped { ", looping" } else { "" })))
            }
            "turn_character" => {
                let (scene, id) = (s("scene").ok_or("scene is required")?, s("item").ok_or("item is required")?);
                let view: Option<View> = match s("view") {
                    Some(v) => Some(serde_json::from_value(json!(v)).map_err(|_| format!("unknown view {v:?}"))?),
                    None => None,
                };
                let mut item = self.ed.project.scene(scene).and_then(|sc| sc.item(id)).cloned().ok_or("no such character in that scene")?;
                let ItemKind::Character(c) = &mut item.kind else { return Err("that item is not a character".into()) };
                if let Some(t) = a.get("auto_turn").and_then(Value::as_bool) {
                    c.auto_turn = t;
                }
                let mut cmds = vec![];
                let msg = match (view, a.get("seconds").and_then(Value::as_f64)) {
                    (Some(v), Some(t)) => {
                        let frame = (t * self.ed.project.fps as f64).round().max(0.0) as u32;
                        // The first turn key also keys the current view at frame 0, so it only turns from then on.
                        if !item.tracks.contains_key("view") && frame > 0 {
                            cmds.push(Command::SetKeys { scene: scene.into(), item: Some(id.into()), path: "view".into(), keys: vec![Key::new(0, json!(c.view))], replace: false });
                        }
                        cmds.push(Command::SetKeys { scene: scene.into(), item: Some(id.into()), path: "view".into(), keys: vec![Key::new(frame, json!(v))], replace: false });
                        format!("{id} turns to {v:?} at {t:.2}s.")
                    }
                    (Some(v), None) => {
                        c.view = v;
                        format!("{id} now faces {v:?}.")
                    }
                    (None, _) => "Updated.".into(),
                };
                cmds.insert(0, Command::UpdateItem { scene: scene.into(), item });
                self.ed.apply(&Command::Batch { commands: cmds }, None)?;
                self.save()?;
                Ok(text(msg))
            }
            "command_reference" => Ok(text(REFERENCE)),
            other => Err(format!("unknown tool {other}")),
        }
    }

    /// Handle one JSON-RPC message; returns the response (None for notifications).
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "frameling", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Frameling makes animated cartoon videos. Start with get_project and get_library. Use script_to_video for a quick first cut, search_parts + create_character to design characters from a description (check with preview_character), list_poses + add_pose_strip to hold poses like sit, kneel or point (with blends; mask 'upper' to gesture while walking), apply_commands for precise edits, say_line for dialogue, set_keys to animate any property over time (movement, rotation, scale, bones, skin colour, outfits, expression, camera), and render_frame to look at your work before export_video."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = msg["params"]["name"].as_str().unwrap_or("");
                let args = msg["params"].get("arguments").cloned().unwrap_or(json!({}));
                Ok(match self.call(name, &args) {
                    Ok(v) => v,
                    Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                })
            }
            m if m.starts_with("notifications/") => return None,
            _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
        };
        id.map(|id| match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
        })
    }
}

pub fn serve(mut server: Server) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => server.handle(&msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(r) = reply {
            writeln!(out, "{r}").map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
