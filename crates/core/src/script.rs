//! Script to video: turn a plain-text screenplay into a project.
//!
//! ```text
//! Title: Coffee break
//! Style: stick                  optional: cast stick figures
//! [office]                      new scene with the "office" background (optionally [office, 6s])
//! Maya (happy, wave): Hi Leo!   a spoken line; words in brackets pick an expression / action
//! Leo (surprised): Oh, hi!
//! > Meanwhile...                a caption card at the top: before a scene's lines
//!                               it opens that scene, after them it opens the
//!                               next one (after the last scene, it ends the movie)
//! ```
//!
//! [`plan`] casts the characters (look and voice stay the same across scenes)
//! and lays out every scene for the frame shape, and returns the lines to
//! voice. The caller voices them (the desktop app with its built-in TTS, the
//! CLI with `frameling_tts`) and places each with `Command::AddLine`; then
//! [`finish`] turns on subtitles, music and the auto camera. Without voices,
//! lines become speech bubbles held for their reading time.

use crate::model::*;
use crate::pack::Library;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

struct Line {
    who: String,
    tags: Vec<String>,
    text: String,
}

struct SceneSpec {
    background: String,
    seconds: Option<f32>,
    lines: Vec<Line>,
    /// Caption cards shown as the scene opens.
    captions: Vec<String>,
    /// Captions written after the last line so far: they open the next scene.
    trailing: Vec<String>,
}

/// What to make, besides the script itself.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    /// Frame size.
    pub width: u32,
    pub height: u32,
    /// Show each line in a speech bubble too (always on without voices).
    pub bubbles: bool,
    /// Voices to cast, in order (the first character gets the first voice).
    pub voices: Vec<String>,
    /// Auto subtitles from the lines.
    pub captions: bool,
    /// Built-in music loop id (`music-lofi`, ...) or empty for none.
    pub music: String,
    /// Cut to whoever speaks.
    pub auto_camera: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { width: 1280, height: 720, bubbles: false, voices: vec![], captions: true, music: "music-lofi".into(), auto_camera: true }
    }
}

/// A line the caller voices and places with `addLine`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineJob {
    pub scene: Id,
    pub speaker: Id,
    pub clip_id: Id,
    pub who: String,
    pub text: String,
    pub voice: Option<String>,
}

/// The planned project, the lines to voice and a log of what was done.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub project: Project,
    pub lines: Vec<LineJob>,
    pub log: Vec<String>,
}

fn presets() -> Vec<(&'static str, Look)> {
    let c = |s: &str| crate::geom::Color::parse(s).unwrap();
    let base = Look::default();
    vec![
        ("Maya", Look { skin: c("#c68b62"), hair: c("#231a16"), hair_style: HairStyle::Long, top: c("#f26b5b"), accessory: Accessory::Glasses, mouth: MouthSet::Lips, ..base.clone() }),
        ("Leo", Look { skin: c("#f3c9a5"), hair: c("#d99a3e"), hair_style: HairStyle::Spiky, top: c("#3fae6a"), bottom: c("#454d72"), build: Build::Broad, accessory: Accessory::Tie, accent: c("#4f7cff"), ..base.clone() }),
        ("Ava", Look { skin: c("#8d5a3b"), hair: c("#1b1311"), hair_style: HairStyle::Curly, top: c("#9b5de5"), bottom: c("#2b2d42"), accessory: Accessory::Bow, accent: c("#ffb703"), build: Build::Slim, mouth: MouthSet::Lips, ..base.clone() }),
        ("Sam", Look { skin: c("#e9b48a"), hair: c("#5a3825"), top: c("#ffb703"), bottom: c("#3a405a"), accessory: Accessory::Cap, accent: c("#e63946"), ..base.clone() }),
        ("Kai", Look { skin: c("#d7a37a"), hair: c("#121212"), hair_style: HairStyle::Bun, top: c("#219ebc"), bottom: c("#23395b"), build: Build::Slim, mouth: MouthSet::Lips, ..base.clone() }),
        ("Ray", Look { skin: c("#b07850"), hair: c("#2d2d2d"), hair_style: HairStyle::Bald, top: c("#e76f51"), bottom: c("#264653"), build: Build::Broad, accessory: Accessory::Glasses, ..base }),
    ]
}

/// Stick-figure cast for "Style: stick" scripts: alternating women and men.
fn stick_presets() -> Vec<(&'static str, Look)> {
    let c = |s: &str| crate::geom::Color::parse(s).unwrap();
    let m = Look::stick(false);
    let f = Look::stick(true);
    vec![
        ("Bea", Look { accent: c("#ef6f6c"), ..f.clone() }),
        ("Alex", Look { hair_style: HairStyle::Short, ..m.clone() }),
        ("Dee", Look { hair_style: HairStyle::Bun, accent: c("#4ea8de"), accessory: Accessory::Bow, ..f.clone() }),
        ("Cal", Look { hair_style: HairStyle::Spiky, accessory: Accessory::Glasses, ..m.clone() }),
        ("Eli", Look { accessory: Accessory::Cap, accent: c("#3fae6a"), ..m.clone() }),
        ("Fay", Look { hair_style: HairStyle::Curly, accent: c("#9b5de5"), ..f }),
        ("Gus", Look { hair_style: HairStyle::Bald, accessory: Accessory::Tie, accent: c("#ef6f6c"), build: Build::Broad, ..m }),
    ]
}

fn parse(src: &str) -> (String, bool, Vec<SceneSpec>) {
    let mut title = "Untitled".to_string();
    let mut stick = false;
    let mut scenes: Vec<SceneSpec> = Vec::new();
    // The first scene was made without a [heading] (lines before any scene).
    let mut implicit = false;
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let lower = line.to_lowercase();
        if lower.starts_with("title:") {
            title = line[6..].trim().to_string();
        } else if lower.starts_with("style:") {
            stick = lower.contains("stick");
        } else if line.starts_with('[') && line.ends_with(']') {
            let inner = line[1..line.len() - 1].trim().trim_start_matches("scene:").trim();
            let mut parts = inner.split(',').map(str::trim);
            let background = parts.next().unwrap_or("studio").to_lowercase().replace(' ', "-");
            let seconds = parts.next().and_then(|s| s.trim_end_matches('s').parse().ok());
            // Captions written after the previous scene's dialogue open this one,
            // as do captions written above the first scene heading.
            let mut captions = scenes.last_mut().map(|p| std::mem::take(&mut p.trailing)).unwrap_or_default();
            if implicit && scenes.len() == 1 && scenes[0].lines.is_empty() {
                captions = scenes.pop().unwrap().captions;
            }
            implicit = false;
            scenes.push(SceneSpec { background, seconds, lines: vec![], captions, trailing: vec![] });
        } else {
            if scenes.is_empty() {
                implicit = true;
                scenes.push(SceneSpec { background: "studio".into(), seconds: None, lines: vec![], captions: vec![], trailing: vec![] });
            }
            let s = scenes.last_mut().unwrap();
            if let Some(c) = line.strip_prefix('>') {
                let c = c.trim().to_string();
                if c.is_empty() {
                    continue;
                }
                if s.lines.is_empty() {
                    s.captions.push(c);
                } else {
                    s.trailing.push(c);
                }
            } else if let Some((head, text)) = line.split_once(':') {
                let (who, tags) = match head.split_once('(') {
                    Some((w, t)) => (w.trim(), t.trim_end_matches(')').split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect()),
                    None => (head.trim(), vec![]),
                };
                if !who.is_empty() && who.len() <= 40 && !text.trim().is_empty() {
                    // More dialogue follows, so captions written mid-scene open this scene.
                    let mid = std::mem::take(&mut s.trailing);
                    s.captions.extend(mid);
                    s.lines.push(Line { who: who.to_string(), tags, text: text.trim().to_string() });
                }
            }
        }
    }
    // Captions after the last scene's dialogue end the movie on a card of their own.
    if let Some(last) = scenes.last_mut() {
        let end = std::mem::take(&mut last.trailing);
        if !end.is_empty() {
            let background = last.background.clone();
            scenes.push(SceneSpec { background, seconds: None, lines: vec![], captions: end, trailing: vec![] });
        }
    }
    scenes.retain(|s| !s.lines.is_empty() || !s.captions.is_empty());
    (title, stick, scenes)
}

fn expression(tag: &str) -> Option<Expression> {
    Some(match tag {
        "neutral" | "calm" => Expression::Neutral,
        "happy" | "smiling" | "excited" | "glad" => Expression::Happy,
        "sad" | "upset" => Expression::Sad,
        "angry" | "mad" | "annoyed" => Expression::Angry,
        "surprised" | "shocked" | "amazed" => Expression::Surprised,
        "thinking" | "curious" | "unsure" => Expression::Thinking,
        "wink" | "winking" | "sly" => Expression::Wink,
        "worried" | "nervous" | "scared" | "anxious" => Expression::Worried,
        "confused" | "puzzled" | "lost" => Expression::Confused,
        _ => return None,
    })
}

fn slug(s: &str) -> String {
    let s: String = s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    s.trim_matches('-').to_string()
}

/// Time to read a line when there is no audio (about 15 characters a second).
pub fn reading_frames(text: &str, fps: u32) -> u32 {
    ((text.chars().count() as f32 / 15.0).max(1.2) * fps as f32) as u32
}

/// Cast and lay out a script. Lines with a voice are returned to be voiced;
/// lines without one become timed speech bubbles.
pub fn plan(src: &str, lib: &Library, opts: &Options) -> Result<Plan, String> {
    let (title, stick, specs) = parse(src);
    if specs.is_empty() {
        return Err("the script has no lines. Write lines like  Maya: Hello!".into());
    }
    let mut log = Vec::new();
    let voices = &opts.voices;
    let bubbles = opts.bubbles || voices.is_empty();
    if voices.is_empty() {
        log.push("no voices: lines are shown as speech bubbles".into());
    }
    let presets = if stick { stick_presets() } else { presets() };
    let mut project = Project { name: title, scenes: vec![], width: opts.width.max(64) & !1, height: opts.height.max(64) & !1, ..Default::default() };
    let fps = project.fps;
    // The visible slice of the world (vertical frames see a narrow middle).
    let k = crate::eval::view_scale(&project);
    let vw = (project.width as f32 / k).min(1280.0);
    let mut cast: BTreeMap<String, (Look, Option<String>)> = BTreeMap::new();
    let mut lines = Vec::new();

    for (si, spec) in specs.iter().enumerate() {
        let scene_id = format!("scene-{}", si + 1);
        let mut scene = Scene::new(&scene_id, &format!("Scene {}", si + 1));
        let bg = if stick && spec.background == "studio" { "stick-paper".to_string() } else { spec.background.clone() };
        if !lib.backgrounds.contains_key(&bg) {
            log.push(format!("unknown background {:?}, using studio", spec.background));
        }
        scene.background.preset = if lib.backgrounds.contains_key(&bg) { bg } else { "studio".into() };
        scene.duration = spec.seconds.map_or(fps, |s| (s * fps as f32) as u32).max(1);
        if si > 0 {
            scene.transition = Transition { kind: TransitionKind::Fade, duration: 12 };
        }
        let mut who: Vec<&str> = Vec::new();
        for l in &spec.lines {
            if !who.iter().any(|w| w.eq_ignore_ascii_case(&l.who)) {
                who.push(&l.who);
            }
        }
        let n = who.len();
        let tall = project.height > project.width;
        let scale = match (tall, n) {
            (true, 0..=2) => 1.0,
            (true, _) => 0.8,
            (false, 0..=3) => 0.95,
            _ => 0.75,
        };
        for (i, name) in who.iter().enumerate() {
            let key = name.to_lowercase();
            let k = cast.len();
            let (look, voice) = cast
                .entry(key)
                .or_insert_with(|| {
                    let look = presets.iter().find(|(p, _)| p.eq_ignore_ascii_case(name)).map(|(_, l)| l.clone()).unwrap_or_else(|| presets[k % presets.len()].1.clone());
                    let voice = (!voices.is_empty()).then(|| voices[k % voices.len()].clone());
                    (look, voice)
                })
                .clone();
            let tags: Vec<String> = spec.lines.iter().find(|l| l.who.eq_ignore_ascii_case(name)).map(|l| l.tags.clone()).unwrap_or_default();
            let expr = tags.iter().find_map(|t| expression(t)).unwrap_or(Expression::Neutral);
            let action = tags.iter().find(|t| lib.actions.contains_key(t.as_str())).cloned().unwrap_or_else(|| "idle".into());
            let f = (i as f32 + 1.0) / (n as f32 + 1.0) - 0.5;
            // Spread across the visible slice, the outermost at 30-40% from the centre.
            let edge = 0.5 - 1.0 / (n as f32 + 1.0);
            let reach = if n <= 2 { 0.27 } else { 0.38 };
            let x = if n == 1 { 640.0 } else { 640.0 + f / edge * reach * vw };
            let view = if n < 2 || f.abs() < 0.05 {
                crate::model::View::Front
            } else if f < 0.0 {
                crate::model::View::FrontRight
            } else {
                crate::model::View::FrontLeft
            };
            scene.items.push(Item {
                id: format!("{}-{}", slug(name), si + 1),
                name: name.to_string(),
                x,
                y: 640.0,
                scale,
                kind: ItemKind::Character(CharacterItem { look, action, expression: expr, voice, view, ..Default::default() }),
                ..Default::default()
            });
        }
        // Caption cards open the scene and fade out once read. A scene with no
        // one in it (the closing card) shows them larger, in the middle.
        let card = spec.captions.iter().map(|c| reading_frames(c, fps)).max().unwrap_or(0) + fps;
        let alone = spec.lines.is_empty();
        let (top, gap, size) = if alone { (360.0 - (spec.captions.len() as f32 - 1.0) * 36.0, 72.0, 44.0) } else { (110.0, 56.0, 30.0) };
        for (ci, c) in spec.captions.iter().enumerate() {
            scene.items.push(Item {
                id: format!("caption-{}-{ci}", si + 1),
                name: "Caption".into(),
                x: 640.0,
                y: top + ci as f32 * gap,
                end: (!alone).then_some(card),
                enter: Effect::Fade,
                exit: Effect::Fade,
                kind: ItemKind::Text(TextItem {
                    text: c.clone(),
                    style: TextStyle::Caption,
                    size,
                    color: crate::geom::Color::WHITE,
                    fill: crate::geom::Color::rgba(0x14, 0x17, 0x26, 0xcc),
                    tail: Tail::None,
                    width: vw * 0.86,
                    pinned: true,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        // Lines: voiced ones go back to the caller; the rest are bubbles.
        let mut t = fps / 3;
        for (li, l) in spec.lines.iter().enumerate() {
            let speaker = format!("{}-{}", slug(&l.who), si + 1);
            let voice = cast.get(&l.who.to_lowercase()).and_then(|c| c.1.clone());
            if voice.is_some() && !bubbles {
                lines.push(LineJob { scene: scene_id.clone(), speaker, clip_id: format!("clip-{}-{li}", si + 1), who: l.who.clone(), text: l.text.clone(), voice });
                continue;
            }
            let frames = reading_frames(&l.text, fps);
            let sp = scene.item(&speaker).unwrap();
            let right = sp.x > 640.0;
            let half = vw / 2.0;
            scene.items.push(Item {
                id: format!("bubble-{}-{li}", si + 1),
                name: format!("{} says", l.who),
                x: (sp.x + if right { -60.0 } else { 60.0 }).clamp(640.0 - half + 170.0, 640.0 + half - 170.0),
                y: 170.0,
                start: t,
                end: Some(t + frames),
                enter: Effect::Pop,
                kind: ItemKind::Text(TextItem { text: l.text.clone(), width: (vw * 0.8).min(320.0), tail: if right { Tail::BottomRight } else { Tail::BottomLeft }, ..Default::default() }),
                ..Default::default()
            });
            if voice.is_some() {
                // Voiced and in a bubble: the caller places the line at the bubble.
                lines.push(LineJob { scene: scene_id.clone(), speaker, clip_id: format!("clip-{}-{li}", si + 1), who: l.who.clone(), text: l.text.clone(), voice });
            }
            t += frames + fps / 5;
        }
        scene.duration = scene.duration.max(t + fps / 2);
        if alone {
            scene.duration = scene.duration.max(card + fps / 2);
        }
        log.push(format!("{}: {} ({}), {} lines", scene.id, scene.background.preset, who.join(", "), spec.lines.len()));
        project.scenes.push(scene);
    }
    Ok(Plan { project, lines, log })
}

/// After the lines are placed: subtitles, music and the auto camera.
pub fn finish(project: &mut Project, opts: &Options) -> Result<(), String> {
    let has_audio = project.scenes.iter().any(|s| !s.audio.is_empty());
    if opts.captions && has_audio {
        let mut c = crate::captions::defaults_for(project.width, project.height);
        c.enabled = true;
        project.captions = c;
    }
    if !opts.music.is_empty() {
        if let Some(m) = crate::sfx::media(&opts.music, project.fps) {
            project.music = Some(Music { media: m.id.clone(), volume: 0.3, ..Default::default() });
            project.media.insert(m.id.clone(), std::sync::Arc::new(m));
        }
    }
    if opts.auto_camera && has_audio {
        let snapshot = project.clone();
        for s in project.scenes.iter_mut() {
            let speakers: std::collections::BTreeSet<&str> = s.audio.iter().filter_map(|c| c.speaker.as_deref()).collect();
            // A single speaker: punch on lines; a conversation: follow the speaker.
            let style = if speakers.len() > 1 || snapshot.height > snapshot.width { crate::camera::AutoStyle::Cuts } else { crate::camera::AutoStyle::Punches };
            let _ = crate::camera::auto(&snapshot, s, style, 1.0);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "Title: Test\n[office]\nMaya (happy, wave): Hi Leo!\nLeo: Oh, hi Maya.\n> Later that day\n[park]\nLeo (sad): I lost my keys.";

    #[test]
    fn plans_scenes_cast_and_lines() {
        let lib = Library::builtin();
        let opts = Options { width: 1080, height: 1920, voices: vec!["v1".into(), "v2".into()], ..Default::default() };
        let p = plan(SRC, lib, &opts).unwrap();
        assert_eq!(p.project.name, "Test");
        assert_eq!(p.project.scenes.len(), 2);
        assert_eq!(p.lines.len(), 3);
        assert_eq!(p.lines[0].voice.as_deref(), Some("v1"));
        assert_eq!(p.lines[2].voice.as_deref(), Some("v2"), "Leo keeps his voice in scene 2");
        // Vertical: both characters inside the narrow visible slice.
        for it in p.project.scenes[0].items.iter().filter(|i| matches!(i.kind, ItemKind::Character(_))) {
            assert!((it.x - 640.0).abs() < 200.0, "{} at {}", it.id, it.x);
        }
    }

    #[test]
    fn without_voices_lines_are_bubbles() {
        let p = plan(SRC, Library::builtin(), &Options::default()).unwrap();
        assert!(p.lines.is_empty());
        let bubbles = p.project.scenes[0].items.iter().filter(|i| i.id.starts_with("bubble-")).count();
        assert_eq!(bubbles, 2);
    }

    fn captions(s: &Scene) -> Vec<(&str, Option<u32>)> {
        s.items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::Text(t) if i.id.starts_with("caption-") => Some((t.text.as_str(), i.end)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn captions_open_the_scene_they_lead_into() {
        let src = "> Monday\n[office]\nMaya: Hi!\n> Later that day\n[park]\n> In the park\nLeo: Hello.\n> Leo waves\nLeo: Bye.\n> The end";
        let p = plan(src, Library::builtin(), &Options::default()).unwrap();
        let s = &p.project.scenes;
        assert_eq!(s.len(), 3, "office, park and a closing card");
        // Above the first heading: opens the office, and leaves it once read.
        assert_eq!(captions(&s[0]).iter().map(|c| c.0).collect::<Vec<_>>(), ["Monday"]);
        assert_eq!(s[0].background.preset, "office");
        let park: Vec<_> = captions(&s[1]).iter().map(|c| c.0).collect();
        assert_eq!(park, ["Later that day", "In the park", "Leo waves"]);
        assert!(captions(&s[1]).iter().all(|c| c.1.is_some()), "cards fade out once read");
        let end = captions(&s[2]);
        assert_eq!(end, [("The end", None)], "the closing card holds to the end");
        assert_eq!(s[2].background.preset, s[1].background.preset);
        assert!(s[2].duration >= 2 * p.project.fps);
    }
}
