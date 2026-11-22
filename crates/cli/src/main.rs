//! `frameling` command-line tool.
//!
//!   frameling demo <out.frameling>                 write the demo project
//!   frameling render <project> <out.mp4|webm> [--width W --height H --crf N --ffmpeg PATH --pack pack.json]
//!   frameling still <project> <frame> <out.png> [--width W --height H --pack pack.json]
//!   frameling apply <project> <commands.json> <out>  apply a JSON array of commands (automation)
//!   frameling voices [--tts DIR]                    list text-to-speech voices
//!   frameling say <project> <scene-id> <character-id> "line" [--voice ID --speed 1.0 --out FILE --tts DIR]
//!                                                   add a spoken, lip-synced line (placed after the last one)
//!   frameling script <screenplay.txt> <out.frameling> [--bubbles]   script-to-video
//!   frameling mcp <project.frameling>              MCP server for AI agents (stdio); see docs/AGENTS.md
//!   frameling sheet <out.png>                       contact sheet of every action and expression (dev aid)

mod mcp;
mod screenplay;

use frameling_core::{demo, evaluate, Command, Editor, Library, Project};
use frameling_export::{export_video, ExportOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn library(args: &[String]) -> Result<Library, String> {
    let mut lib = Library::builtin().clone();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--pack" {
            let p = args.get(i + 1).ok_or("--pack needs a path")?;
            let json = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
            lib.add_pack_json(&json)?;
            i += 1;
        }
        i += 1;
    }
    Ok(lib)
}

/// Bundled voices live in `<dir>/tts` (default: next to the executable).
fn tts_engine(args: &[String]) -> frameling_tts::Tts {
    let base = flag(args, "--tts").map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf()))).unwrap_or_default();
    frameling_tts::Tts::with_bundle(&base)
}

fn load(path: &str) -> Result<Project, String> {
    Project::from_json(&std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    match pos.first().map(|s| s.as_str()) {
        Some("demo") => {
            let out = pos.get(1).ok_or("usage: frameling demo <out.frameling>")?;
            std::fs::write(out, demo::demo_project().to_json_pretty()).map_err(|e| e.to_string())?;
            println!("wrote {out}");
        }
        Some("render") => {
            let (src, out) = (pos.get(1).ok_or("missing project")?, pos.get(2).ok_or("missing output")?);
            let project = load(src)?;
            let lib = library(&args)?;
            let mut opts = ExportOptions::for_project(&project);
            if let Some(w) = flag(&args, "--width") {
                opts.width = w.parse().map_err(|_| "bad --width")?;
                opts.height = flag(&args, "--height")
                    .map(|h| h.parse().map_err(|_| "bad --height"))
                    .transpose()?
                    .unwrap_or(opts.width * project.height / project.width);
            }
            if let Some(c) = flag(&args, "--crf") {
                opts.crf = c.parse().map_err(|_| "bad --crf")?;
            }
            if let Some(f) = flag(&args, "--ffmpeg") {
                opts.ffmpeg = PathBuf::from(f);
            }
            let t0 = Instant::now();
            export_video(&project, &lib, std::path::Path::new(out.as_str()), &opts, |done, total| {
                print!("\rrendering {done}/{total} frames");
                let _ = std::io::stdout().flush();
                true
            })?;
            println!("\nwrote {out} ({}x{}, {:.1}s)", opts.width, opts.height, t0.elapsed().as_secs_f32());
        }
        Some("still") => {
            let src = pos.get(1).ok_or("missing project")?;
            let frame: u32 = pos.get(2).ok_or("missing frame")?.parse().map_err(|_| "bad frame")?;
            let out = pos.get(3).ok_or("missing output")?;
            let project = load(src)?;
            let lib = library(&args)?;
            let w = flag(&args, "--width").and_then(|v| v.parse().ok()).unwrap_or(project.width);
            let h = flag(&args, "--height").and_then(|v| v.parse().ok()).unwrap_or(w * project.height / project.width);
            let png = frameling_render::render_png(&evaluate(&project, &lib, frame), w, h)?;
            std::fs::write(out, png).map_err(|e| e.to_string())?;
            println!("wrote {out}");
        }
        Some("apply") => {
            let (src, cmds, out) = (pos.get(1).ok_or("missing project")?, pos.get(2).ok_or("missing commands")?, pos.get(3).ok_or("missing output")?);
            let mut ed = Editor::new(load(src)?);
            let list: Vec<Command> = serde_json::from_str(&std::fs::read_to_string(cmds.as_str()).map_err(|e| e.to_string())?)
                .map_err(|e| format!("commands: {e}"))?;
            for (i, c) in list.iter().enumerate() {
                ed.apply(c, None).map_err(|e| format!("command {i}: {e}"))?;
            }
            std::fs::write(out.as_str(), ed.project.to_json_pretty()).map_err(|e| e.to_string())?;
            println!("applied {} commands, wrote {out}", list.len());
        }
        Some("voices") => {
            let tts = tts_engine(&args);
            for v in tts.voices() {
                println!("{:<44} {:<8} {}  {}", v.id, v.language, v.name, v.credit);
            }
            if tts.model_dir.is_some() {
                println!("
Built-in voices speak 31 languages: add @<lang> to the id, e.g. supertonic:F1@fr.");
                println!("{}", frameling_tts::SUPERTONIC_TERMS);
            }
        }
        Some("say") => {
            let (src, scene, who) = (pos.get(1).ok_or("missing project")?, pos.get(2).ok_or("missing scene id")?, pos.get(3).ok_or("missing character id")?);
            let text = pos.get(4).ok_or("missing the line to say")?;
            let mut ed = Editor::new(load(src)?);
            let ch = ed.project.scene(scene).and_then(|s| s.item(who)).ok_or("no such character in that scene")?;
            let default_voice = match &ch.kind { frameling_core::ItemKind::Character(c) => c.voice.clone(), _ => return Err("that item is not a character".into()) };
            let tts = tts_engine(&args);
            let voice = flag(&args, "--voice").or(default_voice).or_else(|| tts.voices().first().map(|v| v.id.clone())).ok_or("no voices installed")?;
            let speed: f32 = flag(&args, "--speed").map(|v| v.parse().map_err(|_| "bad --speed")).transpose()?.unwrap_or(1.0);
            let wav = tts.synthesize(text, &voice, speed)?;
            let (frames, envelope) = frameling_core::audio::analyse_wav(&wav, ed.project.fps)?;
            use base64::Engine as _;
            let n = ed.project.media.len() + 1;
            let media = frameling_core::Media {
                id: format!("tts-{n}-{}", std::process::id()),
                name: format!("{}.wav", text.chars().take(24).collect::<String>()),
                mime: "audio/wav".into(),
                data: base64::engine::general_purpose::STANDARD.encode(&wav),
                duration_frames: frames,
                envelope,
                source: Some(frameling_core::MediaSource::Tts { text: text.to_string(), voice: voice.clone(), speed }),
            };
            let clip_id = format!("line-{n}-{}", std::process::id());
            ed.apply(&Command::AddLine { scene: scene.to_string(), clip_id, media, speaker: Some(who.to_string()), start: None }, None)?;
            let out = flag(&args, "--out").unwrap_or_else(|| src.to_string());
            std::fs::write(&out, ed.project.to_json_pretty()).map_err(|e| e.to_string())?;
            println!("{who} says {:?} with {voice} ({:.1}s), wrote {out}", text, frames as f32 / ed.project.fps as f32);
        }
        Some("script") => {
            let (src, out) = (pos.get(1).ok_or("missing screenplay")?, pos.get(2).ok_or("missing output")?);
            let script = std::fs::read_to_string(src.as_str()).map_err(|e| format!("{src}: {e}"))?;
            let (p, log) = screenplay::build(&script, &library(&args)?, &tts_engine(&args), &screenplay::Options { bubbles: args.iter().any(|a| a == "--bubbles") })?;
            for l in log {
                println!("{l}");
            }
            std::fs::write(out.as_str(), p.to_json_pretty()).map_err(|e| e.to_string())?;
            println!("wrote {out} ({:.1}s)", p.total_frames() as f32 / p.fps as f32);
        }
        Some("mcp") => {
            let path = pos.get(1).ok_or("usage: frameling mcp <project.frameling>")?;
            let server = mcp::Server::new(PathBuf::from(path.as_str()), library(&args)?, tts_engine(&args))?;
            mcp::serve(server)?;
        }
        Some("sheet") => {
            let out = pos.get(1).ok_or("missing output")?;
            let lib = library(&args)?;
            let png = frameling_render::render_png(&contact_sheet(&lib), 1600, 1000)?;
            std::fs::write(out.as_str(), png).map_err(|e| e.to_string())?;
            println!("wrote {out}");
        }
        _ => {
            eprintln!("{}", include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
        }
    }
    Ok(())
}

/// Dev aid: every action (at 3 phases) and every expression on one image.
fn contact_sheet(lib: &Library) -> frameling_core::DisplayList {
    use frameling_core::*;
    let mut p = Project { width: 1600, height: 1000, ..Default::default() };
    let mut s = Scene::new("sheet", "sheet");
    s.background.preset = "studio".into();
    s.camera.from = CamKey { x: 800.0, y: 500.0, zoom: 1.0 };
    let exprs = [Expression::Neutral, Expression::Happy, Expression::Sad, Expression::Angry, Expression::Surprised, Expression::Thinking, Expression::Wink];
    let hair = [HairStyle::Short, HairStyle::Long, HairStyle::Spiky, HairStyle::Bun, HairStyle::Curly, HairStyle::Bald, HairStyle::Short];
    let acc = [Accessory::None, Accessory::Glasses, Accessory::Tie, Accessory::Cap, Accessory::Bow, Accessory::None, Accessory::Glasses];
    let mut n = 0;
    for (ai, (id, a)) in lib.actions.iter().enumerate() {
        let col = ai % 13;
        let ch = CharacterItem { action: id.clone(), expression: exprs[ai % exprs.len()], look: Look { hair_style: hair[ai % 7], accessory: acc[ai % 7], ..Default::default() }, ..Default::default() };
        // phase via start offset: start earlier = further into the cycle
        let _ = a;
        s.items.push(Item { id: format!("a{n}"), x: 70.0 + col as f32 * 120.0, y: 440.0, scale: 0.55, start: 0, kind: ItemKind::Character(ch), ..Default::default() });
        n += 1;
    }
    for (i, e) in exprs.iter().enumerate() {
        let ch = CharacterItem { expression: *e, look: Look { hair_style: hair[i], accessory: acc[(i + 3) % 7], build: [Build::Slim, Build::Regular, Build::Broad][i % 3], ..Default::default() }, ..Default::default() };
        s.items.push(Item { id: format!("e{i}"), x: 120.0 + i as f32 * 210.0, y: 960.0, scale: 1.2, kind: ItemKind::Character(ch), ..Default::default() });
    }
    p.scenes = vec![s];
    let frame: u32 = std::env::var("SHEET_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    evaluate(&p, lib, frame)
}
