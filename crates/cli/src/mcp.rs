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
        tool("list_voices", "Text-to-speech voices installed on this machine (ids to use in say_line or a character's voice).", obj(json!({}), &[])),
        tool("apply_commands", "Apply editor commands as one undoable step. Call command_reference for the format. Example: [{\"op\":\"addItem\",\"scene\":\"scene-1\",\"item\":{\"id\":\"desk1\",\"x\":900,\"y\":640,\"kind\":{\"type\":\"prop\",\"prop\":\"desk\"}}}]", obj(json!({ "commands": { "type": "array", "items": { "type": "object" } } }), &["commands"])),
        tool("say_line", "Make a character speak: synthesises the text with a voice, lip-syncs the character, places the line after the previous one and lengthens the scene to fit.", obj(json!({
            "scene": { "type": "string", "description": "scene id" },
            "character": { "type": "string", "description": "character item id" },
            "text": { "type": "string" },
            "voice": { "type": "string", "description": "voice id; defaults to the character's voice" },
            "speed": { "type": "number", "description": "0.5-2, default 1" },
            "start_seconds": { "type": "number", "description": "optional fixed start time in the scene" }
        }), &["scene", "character", "text"])),
        tool("script_to_video", "Replace the project with one built from a screenplay: '[office]' starts a scene, 'Name (happy, wave): line' is dialogue, '> text' is a caption. Characters are cast and voiced automatically.", obj(json!({ "script": { "type": "string" }, "bubbles": { "type": "boolean", "description": "also show lines in speech bubbles" } }), &["script"])),
        tool("render_frame", "Render a frame as a PNG so you can see the result. Give scene + seconds (within that scene), or a global frame number.", obj(json!({
            "scene": { "type": "string" }, "seconds": { "type": "number" }, "frame": { "type": "integer" }, "width": { "type": "integer", "description": "default 640" }
        }), &[])),
        tool("export_video", "Export the whole project to a video file (.mp4 or .webm). Needs FFmpeg.", obj(json!({ "path": { "type": "string" }, "width": { "type": "integer" } }), &["path"])),
        tool("undo", "Undo the last change.", obj(json!({}), &[])),
        tool("redo", "Redo the last undone change.", obj(json!({}), &[])),
        tool("new_project", "Start a new project, blank or from the demo template.", obj(json!({ "template": { "type": "string", "enum": ["blank", "demo"] } }), &[])),
        tool("command_reference", "Full reference for commands, item fields, the screenplay format and the workflow.", obj(json!({}), &[])),
    ])
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
            for it in &s.items {
                let what = match &it.kind {
                    ItemKind::Character(c) => format!("character action={} expression={:?} voice={}", c.action, c.expression, c.voice.as_deref().unwrap_or("-")),
                    ItemKind::Prop(pr) => format!("prop {}", pr.prop),
                    ItemKind::Text(t) => format!("text {:?} {:?}", t.style, t.text),
                };
                let timing = match it.end {
                    Some(e) => format!(" {:.1}-{:.1}s", it.start as f32 / fps, e as f32 / fps),
                    None if it.start > 0 => format!(" from {:.1}s", it.start as f32 / fps),
                    None => String::new(),
                };
                let mv = it.motion.map(|m| format!(" moves to ({:.0},{:.0}) {:.1}-{:.1}s", m.x, m.y, m.start as f32 / fps, m.end as f32 / fps)).unwrap_or_default();
                o += &format!("  - id={:?} {:?} at ({:.0},{:.0}) scale {:.2}{}{}: {}\n", it.id, it.name, it.x, it.y, it.scale, timing, mv, what);
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
                c["expressions"] = json!(["neutral", "happy", "sad", "angry", "surprised", "thinking", "wink"]);
                c["look"] = json!({ "hairStyle": ["short", "spiky", "long", "bun", "curly", "bald"], "build": ["slim", "regular", "broad"], "accessory": ["none", "glasses", "tie", "cap", "bow"] });
                c["textStyles"] = json!(["bubble", "thought", "caption", "title", "plain"]);
                Ok(text(serde_json::to_string_pretty(&c).unwrap()))
            }
            "list_voices" => {
                let v = self.tts.voices();
                Ok(text(if v.is_empty() { "No voices installed.".to_string() } else { serde_json::to_string_pretty(&v).unwrap() }))
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
                "instructions": "Frameling makes animated cartoon videos. Start with get_project and get_library. Use script_to_video for a quick first cut, apply_commands for precise edits, say_line for dialogue, and render_frame to look at your work before export_video."
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
