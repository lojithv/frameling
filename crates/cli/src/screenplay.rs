//! Script-to-video: turn a plain-text screenplay into a project.
//!
//! ```text
//! Title: Coffee break
//! [office]                      new scene with the "office" background (optionally [office, 6s])
//! Maya (happy, wave): Hi Leo!   a spoken line; words in brackets pick an expression / action
//! Leo (surprised): Oh, hi!
//! > Meanwhile...                a caption across the bottom of this scene
//! ```
//! Characters are cast automatically (look and voice stay the same across scenes),
//! lines are voiced with text-to-speech and placed one after another.

use base64::Engine as _;
use frameling_core::*;
use frameling_tts::Tts;
use std::collections::BTreeMap;

pub struct Options {
    /// Also show each line in a speech bubble (always on when no voice is available).
    pub bubbles: bool,
}

struct Line {
    who: String,
    tags: Vec<String>,
    text: String,
}

struct SceneSpec {
    background: String,
    seconds: Option<f32>,
    lines: Vec<Line>,
    captions: Vec<String>,
}

fn presets() -> Vec<(&'static str, Look)> {
    let c = |s: &str| Color::parse(s).unwrap();
    let base = Look::default();
    vec![
        ("Maya", Look { skin: c("#c68b62"), hair: c("#231a16"), hair_style: HairStyle::Long, top: c("#f26b5b"), accessory: Accessory::Glasses, ..base.clone() }),
        ("Leo", Look { skin: c("#f3c9a5"), hair: c("#d99a3e"), hair_style: HairStyle::Spiky, top: c("#3fae6a"), bottom: c("#454d72"), build: Build::Broad, accessory: Accessory::Tie, accent: c("#4f7cff"), ..base.clone() }),
        ("Ava", Look { skin: c("#8d5a3b"), hair: c("#1b1311"), hair_style: HairStyle::Curly, top: c("#9b5de5"), bottom: c("#2b2d42"), accessory: Accessory::Bow, accent: c("#ffb703"), build: Build::Slim, ..base.clone() }),
        ("Sam", Look { skin: c("#e9b48a"), hair: c("#5a3825"), top: c("#ffb703"), bottom: c("#3a405a"), accessory: Accessory::Cap, accent: c("#e63946"), ..base.clone() }),
        ("Kai", Look { skin: c("#d7a37a"), hair: c("#121212"), hair_style: HairStyle::Bun, top: c("#219ebc"), bottom: c("#23395b"), build: Build::Slim, ..base.clone() }),
        ("Ray", Look { skin: c("#b07850"), hair: c("#2d2d2d"), hair_style: HairStyle::Bald, top: c("#e76f51"), bottom: c("#264653"), build: Build::Broad, accessory: Accessory::Glasses, ..base }),
    ]
}

/// Stick-figure cast for "Style: stick" screenplays: alternating men and women,
/// told apart by hair, accessories and dress colour.
fn stick_presets() -> Vec<(&'static str, Look)> {
    let c = |s: &str| Color::parse(s).unwrap();
    let m = Look::stick(false);
    let f = Look::stick(true);
    vec![
        // Women first so they line up with the voice order (F1, M1, F2, M2…).
        ("Bea", Look { accent: c("#ef6f6c"), ..f.clone() }),
        ("Alex", Look { hair_style: HairStyle::Short, ..m.clone() }),
        ("Dee", Look { hair_style: HairStyle::Bun, accent: c("#4ea8de"), accessory: Accessory::Bow, ..f.clone() }),
        ("Cal", Look { hair_style: HairStyle::Spiky, accessory: Accessory::Glasses, ..m.clone() }),
        ("Eli", Look { accessory: Accessory::Cap, accent: c("#3fae6a"), ..m.clone() }),
        ("Fay", Look { hair_style: HairStyle::Curly, accent: c("#9b5de5"), ..f }),
        ("Gus", Look { hair_style: HairStyle::Bald, accessory: Accessory::Tie, accent: c("#ef6f6c"), build: Build::Broad, ..m }),
    ]
}

fn parse(src: &str) -> (String, Vec<SceneSpec>) {
    let mut title = "Untitled".to_string();
    let mut scenes: Vec<SceneSpec> = Vec::new();
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(t) = line.strip_prefix("Title:").or_else(|| line.strip_prefix("title:")) {
            title = t.trim().to_string();
        } else if line.to_lowercase().starts_with("style:") {
            // handled by build(): "Style: stick" casts stick figures
        } else if line.starts_with('[') && line.ends_with(']') {
            let inner = line[1..line.len() - 1].trim().trim_start_matches("scene:").trim();
            let mut parts = inner.split(',').map(str::trim);
            let background = parts.next().unwrap_or("studio").to_lowercase().replace(' ', "-");
            let seconds = parts.next().and_then(|s| s.trim_end_matches('s').parse().ok());
            scenes.push(SceneSpec { background, seconds, lines: vec![], captions: vec![] });
        } else {
            if scenes.is_empty() {
                scenes.push(SceneSpec { background: "studio".into(), seconds: None, lines: vec![], captions: vec![] });
            }
            let s = scenes.last_mut().unwrap();
            if let Some(c) = line.strip_prefix('>') {
                s.captions.push(c.trim().to_string());
            } else if let Some((head, text)) = line.split_once(':') {
                let (who, tags) = match head.split_once('(') {
                    Some((w, t)) => (w.trim(), t.trim_end_matches(')').split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect()),
                    None => (head.trim(), vec![]),
                };
                if !who.is_empty() && !text.trim().is_empty() {
                    s.lines.push(Line { who: who.to_string(), tags, text: text.trim().to_string() });
                }
            }
        }
    }
    (title, scenes)
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

/// Build a project from a screenplay. Returns the project and a log of what was done.
pub fn build(src: &str, lib: &Library, tts: &Tts, opts: &Options) -> Result<(Project, Vec<String>), String> {
    let (title, specs) = parse(src);
    if specs.is_empty() {
        return Err("the script has no scenes or lines".into());
    }
    let mut log = Vec::new();
    // Prefer the built-in neural voices; otherwise the plainest system English voices.
    let voices: Vec<String> = {
        let all = tts.voices();
        let mut built_in: Vec<String> = all.iter().filter(|v| v.engine == "supertonic").map(|v| v.id.clone()).collect();
        // Alternate F1, M1, F2, M2… so a two-hander gets two clearly different voices.
        built_in.sort_by_key(|id| (id.chars().last(), id.clone()));
        if !built_in.is_empty() {
            built_in
        } else {
            let mut en: Vec<&frameling_tts::Voice> = all.iter().filter(|v| v.language.starts_with("en")).collect();
            let rank = |l: &str| match l.to_lowercase().as_str() { "en-us" => 0, "en-gb" => 1, "en" => 2, "en-au" | "en-ca" => 3, _ => 9 };
            en.sort_by_key(|v| rank(&v.language));
            let en: Vec<String> = en.into_iter().map(|v| v.id.clone()).collect();
            if en.is_empty() { all.into_iter().map(|v| v.id).collect() } else { en }
        }
    };
    let bubbles = opts.bubbles || voices.is_empty();
    if voices.is_empty() {
        log.push("no text-to-speech voices found: lines are shown as speech bubbles only".into());
    }
    let stick = src.lines().any(|l| {
        let l = l.trim().to_lowercase();
        l.starts_with("style:") && l.contains("stick")
    });
    let presets = if stick { stick_presets() } else { presets() };
    let mut cast: BTreeMap<String, (Look, Option<String>)> = BTreeMap::new();
    let mut ed = Editor::new(Project { name: title, scenes: vec![], ..Default::default() });
    let fps = ed.project.fps;

    for (si, spec) in specs.iter().enumerate() {
        let scene_id = format!("scene-{}", si + 1);
        let mut scene = Scene::new(&scene_id, &format!("Scene {}", si + 1));
        if !lib.backgrounds.contains_key(&spec.background) {
            log.push(format!("unknown background {:?}, using studio", spec.background));
        }
        scene.background.preset = if lib.backgrounds.contains_key(&spec.background) { spec.background.clone() } else { "studio".into() };
        scene.duration = spec.seconds.map_or(fps, |s| (s * fps as f32) as u32);
        if si > 0 {
            scene.transition = Transition { kind: TransitionKind::Fade, duration: 12 };
        }
        // Who is in this scene, in order of first line.
        let mut who: Vec<&str> = Vec::new();
        for l in &spec.lines {
            if !who.iter().any(|w| w.eq_ignore_ascii_case(&l.who)) {
                who.push(&l.who);
            }
        }
        let n = who.len();
        for (i, name) in who.iter().enumerate() {
            let key = name.to_lowercase();
            let k = cast.len();
            let (look, voice) = cast
                .entry(key.clone())
                .or_insert_with(|| {
                    let look = presets.iter().find(|(p, _)| p.eq_ignore_ascii_case(name)).map(|(_, l)| l.clone()).unwrap_or_else(|| presets[k % presets.len()].1.clone());
                    let voice = (!voices.is_empty()).then(|| voices[k % voices.len()].clone());
                    (look, voice)
                })
                .clone();
            let first_tags: Vec<String> = spec.lines.iter().find(|l| l.who.eq_ignore_ascii_case(name)).map(|l| l.tags.clone()).unwrap_or_default();
            let expr = first_tags.iter().find_map(|t| expression(t)).unwrap_or(Expression::Neutral);
            let action = first_tags.iter().find(|t| lib.actions.contains_key(t.as_str())).cloned().unwrap_or_else(|| "idle".into());
            let x = 1280.0 * (i as f32 + 1.0) / (n as f32 + 1.0);
            scene.items.push(Item {
                id: format!("{}-{}", slug(name), si + 1),
                name: name.to_string(),
                x,
                y: 640.0,
                scale: if n > 3 { 0.75 } else { 0.95 },
                flip: x > 640.0,
                kind: ItemKind::Character(CharacterItem { look, action, expression: expr, voice, ..Default::default() }),
                ..Default::default()
            });
        }
        for (ci, c) in spec.captions.iter().enumerate() {
            scene.items.push(Item {
                id: format!("caption-{}-{ci}", si + 1),
                name: "Caption".into(),
                x: 640.0,
                y: 670.0 - ci as f32 * 56.0,
                enter: Effect::Fade,
                kind: ItemKind::Text(TextItem { text: c.clone(), style: TextStyle::Caption, size: 30.0, color: Color::WHITE, fill: Color::rgba(0x14, 0x17, 0x26, 0xcc), tail: Tail::None, width: 900.0, ..Default::default() }),
                ..Default::default()
            });
        }
        ed.apply(&Command::AddScene { scene, index: None }, None)?;

        for (li, l) in spec.lines.iter().enumerate() {
            let char_id = format!("{}-{}", slug(&l.who), si + 1);
            let voice = cast.get(&l.who.to_lowercase()).and_then(|c| c.1.clone());
            let media_id = format!("line-{}-{li}", si + 1);
            let (media, frames) = match voice.as_deref().map(|v| tts.synthesize(&l.text, v, 1.0)) {
                Some(Ok(wav)) => {
                    let (frames, envelope) = audio::analyse_wav(&wav, fps)?;
                    let m = Media {
                        id: media_id.clone(),
                        name: format!("{}: {}", l.who, l.text.chars().take(24).collect::<String>()),
                        mime: "audio/wav".into(),
                        data: base64::engine::general_purpose::STANDARD.encode(&wav),
                        duration_frames: frames,
                        envelope,
                        source: Some(MediaSource::Tts { text: l.text.clone(), voice: voice.clone().unwrap(), speed: 1.0 }),
                        size: None,
                    };
                    (Some(m), frames)
                }
                Some(Err(e)) => {
                    log.push(format!("could not voice {:?}: {e}", l.text));
                    (None, reading_frames(&l.text, fps))
                }
                None => (None, reading_frames(&l.text, fps)),
            };
            let start = match media {
                Some(m) => {
                    let clip_id = format!("clip-{}-{li}", si + 1);
                    ed.apply(&Command::AddLine { scene: scene_id.clone(), clip_id: clip_id.clone(), media: m, speaker: Some(char_id.clone()), start: None }, None)?;
                    ed.project.scene(&scene_id).and_then(|s| s.audio.iter().find(|c| c.id == clip_id)).map(|c| c.start).unwrap_or(0)
                }
                None => {
                    // No audio: reserve reading time on the scene instead.
                    let s = ed.project.scene(&scene_id).unwrap();
                    let last_end = s.items.iter().filter(|i| i.id.starts_with("bubble-")).filter_map(|i| i.end).max().unwrap_or(fps / 3);
                    let mut sc = s.clone();
                    sc.duration = sc.duration.max(last_end + frames + fps / 2);
                    ed.apply(&Command::UpdateScene { scene: sc }, None)?;
                    last_end + fps / 5
                }
            };
            if bubbles {
                let s = ed.project.scene(&scene_id).unwrap();
                let speaker = s.item(&char_id).unwrap();
                let right = speaker.x > 640.0;
                ed.apply(
                    &Command::AddItem {
                        scene: scene_id.clone(),
                        item: Item {
                            id: format!("bubble-{}-{li}", si + 1),
                            name: format!("{} says", l.who),
                            x: (speaker.x + if right { -60.0 } else { 60.0 }).clamp(220.0, 1060.0),
                            y: 150.0,
                            start,
                            end: Some(start + frames.max(fps)),
                            enter: Effect::Pop,
                            kind: ItemKind::Text(TextItem { text: l.text.clone(), tail: if right { Tail::BottomRight } else { Tail::BottomLeft }, ..Default::default() }),
                            ..Default::default()
                        },
                        index: None,
                    },
                    None,
                )?;
            }
        }
        let s = ed.project.scene(&scene_id).unwrap();
        log.push(format!("{}: {} ({}), {} lines, {:.1}s", s.id, s.background.preset, who.join(", "), spec.lines.len(), s.duration as f32 / fps as f32));
    }
    Ok((ed.project, log))
}

/// Time to read a line when there is no audio (about 15 characters a second).
fn reading_frames(text: &str, fps: u32) -> u32 {
    ((text.chars().count() as f32 / 15.0).max(1.2) * fps as f32) as u32
}
