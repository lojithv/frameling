//! Script-to-video for the CLI and MCP: plan the project with
//! `frameling_core::script`, voice every line with the bundled TTS, then add
//! subtitles, music and the auto camera. The screenplay format is documented
//! in crates/core/src/script.rs.

use base64::Engine as _;
use frameling_core::script::{self, LineJob};
use frameling_core::*;
use frameling_tts::Tts;

pub struct Options {
    /// Also show each line in a speech bubble (always on when no voice is available).
    pub bubbles: bool,
    /// Frame shape: vertical, wide or square.
    pub shape: String,
    /// Built-in music loop id, or empty for none.
    pub music: String,
}

impl Default for Options {
    fn default() -> Self {
        Options { bubbles: false, shape: "wide".into(), music: "music-lofi".into() }
    }
}

/// Voices to cast: the built-in neural voices (alternating F1, M1, F2...), or
/// the plainest English system voices.
fn cast_voices(tts: &Tts) -> Vec<String> {
    let all = tts.voices();
    let mut built_in: Vec<String> = all.iter().filter(|v| v.engine == "supertonic").map(|v| v.id.clone()).collect();
    built_in.sort_by_key(|id| (id.chars().last(), id.clone()));
    if !built_in.is_empty() {
        return built_in;
    }
    let mut en: Vec<&frameling_tts::Voice> = all.iter().filter(|v| v.language.starts_with("en")).collect();
    let rank = |l: &str| match l.to_lowercase().as_str() {
        "en-us" => 0,
        "en-gb" => 1,
        "en" => 2,
        "en-au" | "en-ca" => 3,
        _ => 9,
    };
    en.sort_by_key(|v| rank(&v.language));
    let en: Vec<String> = en.into_iter().map(|v| v.id.clone()).collect();
    if en.is_empty() {
        all.into_iter().map(|v| v.id).collect()
    } else {
        en
    }
}

/// Build a project from a screenplay. Returns the project and a log of what was done.
pub fn build(src: &str, lib: &Library, tts: &Tts, opts: &Options) -> Result<(Project, Vec<String>), String> {
    let (width, height) = templates::size(&opts.shape);
    let sopts = script::Options { width, height, bubbles: opts.bubbles, voices: cast_voices(tts), music: opts.music.clone(), ..Default::default() };
    let plan = script::plan(src, lib, &sopts)?;
    let mut log = plan.log;
    let mut ed = Editor::new(plan.project);
    let fps = ed.project.fps;
    for job in &plan.lines {
        if let Err(e) = voice_line(&mut ed, tts, job, fps) {
            log.push(format!("could not voice {:?}: {e}", job.text));
        }
    }
    let mut p = ed.project;
    script::finish(&mut p, &sopts)?;
    for s in &p.scenes {
        log.push(format!("{}: {:.1}s, {} lines", s.id, s.duration as f32 / fps as f32, s.audio.len()));
    }
    Ok((p, log))
}

fn voice_line(ed: &mut Editor, tts: &Tts, job: &LineJob, fps: u32) -> Result<(), String> {
    let voice = job.voice.clone().ok_or("no voice")?;
    let wav = tts.synthesize(&job.text, &voice, 1.0)?;
    let (frames, envelope) = audio::analyse_wav(&wav, fps)?;
    let media = Media {
        id: format!("media-{}", job.clip_id),
        name: format!("{}: {}", job.who, job.text.chars().take(24).collect::<String>()),
        mime: "audio/wav".into(),
        data: base64::engine::general_purpose::STANDARD.encode(&wav),
        duration_frames: frames,
        envelope,
        source: Some(MediaSource::Tts { text: job.text.clone(), voice, speed: 1.0 }),
        size: None,
    };
    // In bubble mode the line starts with its bubble.
    let bubble_start = ed.project.scene(&job.scene).and_then(|s| s.items.iter().find(|i| i.id == job.clip_id.replace("clip-", "bubble-")).map(|i| i.start));
    ed.apply(&Command::AddLine { scene: job.scene.clone(), clip_id: job.clip_id.clone(), media, speaker: Some(job.speaker.clone()), start: bubble_start }, None)
}
