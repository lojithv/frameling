//! Auto subtitles.
//!
//! Every voice line knows what it says (the typed text of a text-to-speech
//! line, or a caption typed for a recording) and has a loudness envelope, one
//! value per frame. From those two, [`time_words`] estimates when each word
//! is spoken: pauses in the envelope are matched to punctuation, and within a
//! stretch of speech the time is shared out by how long each word is. That is
//! deterministic and needs no speech recogniser, so preview and export agree.
//!
//! Words are then grouped into short captions ([`pages`]) and drawn on top of
//! the frame in screen space ([`caption_ops`]), with the spoken word
//! highlighted.

use crate::display::{Op, Path};
use crate::geom::{Affine, Color, Ease};
use crate::model::{AudioClip, CaptionStyle, Captions, Media, Project, Scene};
use crate::text;

/// A word and when it is spoken, in frames of its media.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f32,
    pub end: f32,
}

/// A group of words shown together.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub first: usize,
    pub last: usize,
    pub start: f32,
    pub end: f32,
}

/// Shortest silence (frames) counted as a pause between words.
const MIN_GAP: usize = 3;
/// A pause this long (frames) always starts a new caption.
const PAGE_GAP: f32 = 9.0;
/// How long the last caption of a line stays after the last word (frames).
const HOLD: f32 = 8.0;

fn ends_phrase(w: &str) -> bool {
    w.trim_end_matches(['"', '\'', ')', ']', '”', '’']).ends_with(['.', ',', '!', '?', ';', ':', '…', '—'])
}

/// Rough speaking length of a word (letters, with digits read out longer).
fn weight(w: &str) -> f32 {
    let letters = w.chars().filter(|c| c.is_alphabetic()).count() as f32;
    let digits = w.chars().filter(|c| c.is_ascii_digit()).count() as f32;
    1.2 + letters * 0.55 + digits * 1.6
}

/// Spread `words` over the voiced frames of `[a, b)`, by weight.
fn spread(words: &[&str], voiced: &[bool], a: usize, b: usize, out: &mut Vec<Word>) {
    if words.is_empty() {
        return;
    }
    let mut frames: Vec<usize> = (a..b).filter(|&i| voiced.get(i).copied().unwrap_or(false)).collect();
    if frames.len() < words.len() {
        frames = (a..b.max(a + 1)).collect();
    }
    let n = frames.len() as f32;
    let total: f32 = words.iter().map(|w| weight(w)).sum();
    let at = |frac: f32| -> f32 {
        // Position `frac` of the way through the voiced frames, as a frame number.
        let pos = (frac * n).clamp(0.0, n);
        let i = pos.floor() as usize;
        if i >= frames.len() {
            return *frames.last().unwrap() as f32 + 1.0;
        }
        frames[i] as f32 + pos.fract()
    };
    let mut acc = 0.0;
    for w in words {
        let s = at(acc / total);
        acc += weight(w);
        // End at the end of the last voiced frame used, not across a pause.
        let e_pos = (acc / total * n).clamp(0.0, n);
        let last = ((e_pos.ceil() as usize).max(1) - 1).min(frames.len() - 1);
        let e = (frames[last] as f32 + 1.0).max(s + 0.5);
        out.push(Word { text: w.to_string(), start: s, end: e });
    }
}

/// When each word of `text` is spoken, from the media's loudness envelope
/// (one value per frame; `frames` = the media's length).
pub fn time_words(text: &str, envelope: &[f32], frames: u32) -> Vec<Word> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![];
    }
    let n = (frames as usize).max(envelope.len()).max(1);
    let peak = envelope.iter().cloned().fold(0.0f32, f32::max);
    let thr = (peak * 0.12).max(0.015);
    let voiced: Vec<bool> = (0..n).map(|i| envelope.get(i).copied().unwrap_or(0.0) > thr).collect();
    let first = voiced.iter().position(|&v| v).unwrap_or(0);
    let last = voiced.iter().rposition(|&v| v).map_or(n, |i| i + 1);
    if last <= first {
        let mut out = vec![];
        spread(&words, &vec![true; n], 0, n, &mut out);
        return out;
    }

    // Pauses inside the speech.
    let mut gaps: Vec<(usize, usize)> = vec![];
    let mut i = first;
    while i < last {
        if !voiced[i] {
            let s = i;
            while i < last && !voiced[i] {
                i += 1;
            }
            if i - s >= MIN_GAP {
                gaps.push((s, i));
            }
        } else {
            i += 1;
        }
    }

    // Phrase breaks after punctuation; match the longest pauses to them.
    let breaks: Vec<usize> = (0..words.len() - 1).filter(|&k| ends_phrase(words[k])).collect();
    let mut out = Vec::with_capacity(words.len());
    if !breaks.is_empty() && gaps.len() >= breaks.len() {
        let mut chosen = gaps.clone();
        chosen.sort_by(|a, b| (b.1 - b.0).cmp(&(a.1 - a.0)).then(a.0.cmp(&b.0)));
        chosen.truncate(breaks.len());
        chosen.sort();
        let mut seg_start = first;
        let mut w0 = 0;
        for (gi, &(ga, gb)) in chosen.iter().enumerate() {
            let w1 = breaks[gi] + 1;
            spread(&words[w0..w1], &voiced, seg_start, ga, &mut out);
            seg_start = gb;
            w0 = w1;
        }
        spread(&words[w0..], &voiced, seg_start, last, &mut out);
    } else {
        spread(&words, &voiced, first, last, &mut out);
    }
    out
}

/// Group words into captions of at most `max_words`, breaking at punctuation
/// and pauses. A caption shows until the next one starts (or a little after
/// its last word, before a pause).
pub fn pages(words: &[Word], max_words: u32) -> Vec<Page> {
    let max = max_words.clamp(1, 12) as usize;
    let mut out: Vec<Page> = vec![];
    let mut first = 0;
    for i in 0..words.len() {
        let count = i + 1 - first;
        let next_gap = words.get(i + 1).map(|n| n.start - words[i].end);
        let brk = count >= max || ends_phrase(&words[i].text) || next_gap.is_none_or(|g| g > PAGE_GAP);
        if brk {
            out.push(Page { first, last: i, start: words[first].start, end: words[i].end + HOLD });
            first = i + 1;
        }
    }
    for k in 0..out.len().saturating_sub(1) {
        let next = out[k + 1].start;
        out[k].end = out[k].end.min(next);
        if next - words[out[k].last].end <= PAGE_GAP {
            out[k].end = next;
        }
    }
    out
}

/// Timed words of a clip, or none when it has no subtitle.
pub fn clip_words(clip: &AudioClip, media: &Media) -> Vec<Word> {
    match clip.caption_text(media) {
        Some(t) => time_words(t, &media.envelope, media.duration_frames),
        None => vec![],
    }
}

/// The caption showing at a scene-local frame: the clip's words, the page
/// and the playhead in media frames.
fn active(project: &Project, scene: &Scene, local: u32) -> Option<(Vec<Word>, Page, f32)> {
    let mut found = None;
    for clip in &scene.audio {
        let Some(media) = project.media.get(&clip.media) else { continue };
        if local < clip.start {
            continue;
        }
        let play = clip.play_frames(media) as f32;
        let since = (local - clip.start) as f32;
        if since >= play + HOLD {
            continue;
        }
        let words = clip_words(clip, media);
        if words.is_empty() {
            continue;
        }
        let t = since + clip.trim_start as f32 + 0.5;
        let limit = clip.trim_start as f32 + play + HOLD;
        let Some(page) = pages(&words, project.captions.words)
            .into_iter()
            .filter(|p| p.start >= clip.trim_start as f32 - 0.5)
            .find(|p| t >= p.start && t < p.end.min(limit))
        else {
            continue;
        };
        found = Some((words, page, t));
    }
    found
}

/// Caption overlay for a scene at a local frame, in output pixels.
pub fn caption_ops(project: &Project, scene: &Scene, local: u32) -> Vec<Op> {
    let cap = &project.captions;
    if !cap.enabled {
        return vec![];
    }
    let Some((words, page, t)) = active(project, scene, local) else { return vec![] };
    let (w, h) = (project.width as f32, project.height as f32);
    let k = w.min(h) / 720.0;
    let size = cap.size.clamp(8.0, 200.0) * k;
    let shown: Vec<String> = words[page.first..=page.last]
        .iter()
        .map(|wd| if cap.uppercase { wd.text.to_uppercase() } else { wd.text.clone() })
        .collect();
    let lay = text::layout_spaced(&shown.join(" "), cap.font, size, w * 0.86, size * 0.12);
    let speaking = (page.first..=page.last).rev().find(|&i| words[i].start <= t).map(|i| i - page.first);
    let fps_k = project.fps.max(1) as f32 / 30.0;

    let stroke_w = cap.stroke_width.max(0.0) * k * 2.0;
    let mut under: Vec<Op> = vec![];
    let mut strokes: Vec<Op> = vec![];
    let mut fills: Vec<Op> = vec![];

    // Bounds of one word, for the pill.
    let word_rect = |wi: usize| {
        let gs: Vec<&text::Glyph> = lay.glyphs.iter().filter(|g| g.word == wi && !g.ch.is_whitespace()).collect();
        let x0 = gs.iter().map(|g| g.x).fold(f32::MAX, f32::min);
        let x1 = gs.iter().map(|g| g.x + g.advance).fold(f32::MIN, f32::max);
        let base = gs.first().map_or(0.0, |g| g.baseline);
        (x0, base - lay.ascent * 0.92, x1 - x0, lay.ascent * 0.92 + lay.descent * 0.9)
    };

    match cap.style {
        CaptionStyle::Boxed => {
            let (px, py) = (size * 0.45, size * 0.28);
            under.push(Op::fill(
                Path::round_rect(-lay.width / 2.0 - px, -lay.height / 2.0 - py, lay.width + 2.0 * px, lay.height + 2.0 * py, size * 0.3),
                cap.box_color,
            ));
        }
        CaptionStyle::Pill => {
            if let Some(wi) = speaking {
                let (x, y, ww, hh) = word_rect(wi);
                let (px, py) = (size * 0.18, size * 0.1);
                under.push(Op::fill(Path::round_rect(x - px, y - py, ww + 2.0 * px, hh + 2.0 * py, size * 0.22), cap.highlight));
            }
        }
        _ => {}
    }

    for wi in 0..lay.words {
        let word = &words[page.first + wi];
        let is_now = speaking == Some(wi);
        let color = match cap.style {
            CaptionStyle::Pop | CaptionStyle::Boxed if is_now => cap.highlight,
            CaptionStyle::Karaoke if word.start <= t => cap.highlight,
            _ => cap.color,
        };
        let mut path = Path::new();
        for g in lay.glyphs.iter().filter(|g| g.word == wi) {
            path.extend(&g.path);
        }
        // The spoken word grows a little (pop style), easing in quickly.
        let grow = if cap.style == CaptionStyle::Pop && is_now {
            let p = ((t - word.start) / (3.0 * fps_k)).clamp(0.0, 1.0);
            1.0 + 0.1 * Ease::Out.apply(p)
        } else {
            1.0
        };
        let tf = if grow != 1.0 {
            let (x, y, ww, hh) = word_rect(wi);
            let (cx, cy) = (x + ww / 2.0, y + hh / 2.0);
            Affine::translate(cx, cy).then(&Affine::scale(grow, grow)).then(&Affine::translate(-cx, -cy))
        } else {
            Affine::IDENTITY
        };
        if stroke_w > 0.0 && cap.style != CaptionStyle::Boxed {
            strokes.push(Op::group(tf, 1.0, vec![Op::stroke(path.clone(), cap.stroke, stroke_w)]));
        }
        if cap.style == CaptionStyle::Clean {
            let off = size * 0.06;
            under.push(Op::group(Affine::translate(off * 0.4, off).then(&tf), 0.45, vec![Op::fill(path.clone(), Color::BLACK)]));
        }
        fills.push(Op::group(tf, 1.0, vec![Op::fill(path, color)]));
    }

    // The group pops in when it appears.
    let (mut s, mut alpha) = (1.0, 1.0);
    if cap.pop {
        let p = ((t - page.start) / (5.0 * fps_k)).clamp(0.0, 1.0);
        s = 0.72 + 0.28 * Ease::Back.apply(p);
        alpha = (p * 2.5).min(1.0);
    }
    let cy = (cap.y.clamp(0.0, 1.0) * h).clamp(lay.height / 2.0 + size * 0.4, h - lay.height / 2.0 - size * 0.4);
    let place = Affine::translate(w / 2.0, cy).then(&Affine::scale(s, s));
    let mut all = under;
    all.extend(strokes);
    all.extend(fills);
    vec![Op::group(place, alpha, all)]
}

fn srt_time(secs: f64) -> String {
    let ms = (secs.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

/// The whole movie's subtitles as an SRT file (one cue per caption).
pub fn srt(project: &Project) -> String {
    let fps = project.fps.max(1) as f64;
    let mut cues: Vec<(f64, f64, String)> = vec![];
    for (scene, start) in project.scenes.iter().zip(project.scene_starts()) {
        for clip in &scene.audio {
            let Some(media) = project.media.get(&clip.media) else { continue };
            let words = clip_words(clip, media);
            let play = clip.play_frames(media).min(scene.duration.saturating_sub(clip.start)) as f32;
            for p in pages(&words, project.captions.words) {
                let a = p.start - clip.trim_start as f32;
                let b = (p.end - clip.trim_start as f32).min(play + HOLD).min((scene.duration - clip.start.min(scene.duration)) as f32);
                if a < 0.0 || b <= a {
                    continue;
                }
                let text: Vec<&str> = words[p.first..=p.last].iter().map(|w| w.text.as_str()).collect();
                let at = (start + clip.start) as f64;
                cues.push(((at + a as f64) / fps, (at + b as f64) / fps, text.join(" ")));
            }
        }
    }
    cues.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = String::new();
    for (i, (a, b, t)) in cues.iter().enumerate() {
        out.push_str(&format!("{}\n{} --> {}\n{}\n\n", i + 1, srt_time(*a), srt_time(*b), t));
    }
    out
}

/// Caption settings with the defaults for a frame shape (vertical video sits
/// its captions a little higher, clear of on-screen buttons).
pub fn defaults_for(width: u32, height: u32) -> Captions {
    let mut c = Captions::default();
    if height > width {
        c.y = 0.7;
        c.size = 50.0;
    } else if height == width {
        c.y = 0.8;
    } else {
        c.y = 0.84;
        c.size = 44.0;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(parts: &[(usize, f32)]) -> Vec<f32> {
        parts.iter().flat_map(|&(n, v)| std::iter::repeat_n(v, n)).collect()
    }

    #[test]
    fn pauses_line_up_with_punctuation() {
        // 5 frames lead-in, speech, a 10-frame pause, more speech.
        let e = env(&[(5, 0.0), (30, 0.8), (10, 0.0), (20, 0.7), (5, 0.0)]);
        let w = time_words("Hello there, my friend.", &e, e.len() as u32);
        assert_eq!(w.len(), 4);
        assert!(w[0].start >= 5.0 && w[0].start < 6.0, "{w:?}");
        assert!(w[1].end <= 35.5, "the second word ends before the pause: {w:?}");
        assert!(w[2].start >= 45.0, "the third word starts after the pause: {w:?}");
        assert!(w[3].end <= 65.5);
        for p in w.windows(2) {
            assert!(p[0].start < p[1].start);
        }
    }

    #[test]
    fn pages_break_at_punctuation_and_size() {
        let e = env(&[(60, 0.8)]);
        let w = time_words("one two three four five. six", &e, 60);
        let p = pages(&w, 3);
        let groups: Vec<(usize, usize)> = p.iter().map(|p| (p.first, p.last)).collect();
        assert_eq!(groups, vec![(0, 2), (3, 4), (5, 5)]);
        assert_eq!(p[0].end, p[1].start, "back to back without a pause");
    }

    #[test]
    fn srt_has_cues() {
        use crate::model::*;
        let mut p = Project::default();
        let e = env(&[(30, 0.8)]);
        p.media.insert(
            "m".into(),
            std::sync::Arc::new(Media {
                id: "m".into(),
                duration_frames: 30,
                envelope: e,
                source: Some(MediaSource::Tts { text: "Hi there".into(), voice: "x".into(), speed: 1.0 }),
                ..Default::default()
            }),
        );
        p.scenes[0].audio.push(AudioClip { id: "c".into(), media: "m".into(), start: 30, ..Default::default() });
        let s = srt(&p);
        assert!(s.starts_with("1\n00:00:01,000 --> "), "{s}");
        assert!(s.contains("Hi there"));
    }
}
