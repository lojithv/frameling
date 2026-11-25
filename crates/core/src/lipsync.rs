//! Lip sync with mouth shapes (visemes).
//!
//! Every voice line has a loudness envelope (one value per frame) and, for a
//! typed line or a captioned recording, its words. [`timeline`] turns those
//! into one mouth shape per frame of the media:
//!
//! - With words: each word is timed to the audio (the same timing the auto
//!   subtitles use, see `captions::time_words`), spelled out as rough sounds
//!   (`m`/`b`/`p` close the lips, `f`/`v` bite the lip, `o`, `oo`/`w` round
//!   them, vowels open the jaw, `l`/`th` show the tongue...) and those shapes
//!   are spread over the word's frames. Loudness then scales how wide vowels
//!   open.
//! - Without words (an imported sound): shapes are picked from the loudness
//!   alone, closing the lips between syllables.
//!
//! Like everything in the evaluator this is a pure function of the project,
//! so preview and export always agree. No speech recogniser is needed.

use crate::captions;
use crate::geom::hash_str;
use crate::model::{AudioClip, Media};
use serde::{Deserialize, Serialize};

/// The nine standard mouth shapes. Every mouth set (and the stick figure)
/// draws all of them, so any set fits any character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Viseme {
    /// Not speaking: the expression's own mouth.
    #[default]
    Rest,
    /// Lips pressed together: M, B, P.
    Mbp,
    /// Teeth together, lips slightly apart: S, T, K, N, D, EE...
    Ee,
    /// Open: E as in "bed", short I.
    Eh,
    /// Wide open: A as in "father", I as in "high".
    Ai,
    /// Round: O as in "go".
    O,
    /// Pursed: OO, W, Q.
    U,
    /// Lower lip under the top teeth: F, V.
    Fv,
    /// Open with the tongue up behind the teeth: L, TH.
    L,
}

impl Viseme {
    pub const ALL: [Viseme; 9] = [Viseme::Rest, Viseme::Mbp, Viseme::Ee, Viseme::Eh, Viseme::Ai, Viseme::O, Viseme::U, Viseme::Fv, Viseme::L];

    pub fn name(self) -> &'static str {
        match self {
            Viseme::Rest => "rest",
            Viseme::Mbp => "mbp",
            Viseme::Ee => "ee",
            Viseme::Eh => "eh",
            Viseme::Ai => "ai",
            Viseme::O => "o",
            Viseme::U => "u",
            Viseme::Fv => "fv",
            Viseme::L => "l",
        }
    }

    /// An open vowel shape (its size follows the loudness).
    fn is_vowel(self) -> bool {
        matches!(self, Viseme::Eh | Viseme::Ai | Viseme::O)
    }
}

/// How a character's mouth follows its voice lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LipSync {
    /// Mouth shapes timed to the words (the default).
    #[default]
    Shapes,
    /// Just opens and closes with the loudness.
    Simple,
    /// The mouth doesn't move (the character still gestures).
    Off,
}

/// A rough sound of a word, with how long it lasts relative to the others.
fn sounds(word: &str) -> Vec<(Viseme, f32)> {
    use Viseme::*;
    let w: Vec<char> = word.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect();
    let mut out: Vec<(Viseme, f32)> = Vec::new();
    let mut push = |v: Viseme, weight: f32| match out.last_mut() {
        Some(last) if last.0 == v => last.1 += weight * 0.6,
        _ => out.push((v, weight)),
    };
    let n = w.len();
    let mut i = 0;
    while i < n {
        let c = w[i];
        let next = w.get(i + 1).copied();
        let pair = next.map(|d| [c, d]);
        // Two letters, one sound.
        let two = match pair {
            Some(['t', 'h']) => Some((L, 0.8)),
            Some(['s', 'h']) | Some(['c', 'h']) => Some((U, 0.8)),
            Some(['p', 'h']) => Some((Fv, 0.8)),
            Some(['w', 'h']) | Some(['q', 'u']) => Some((U, 0.7)),
            Some(['o', 'o']) => Some((U, 1.2)),
            Some(['e', 'e']) | Some(['e', 'a']) | Some(['i', 'e']) => Some((Ee, 1.1)),
            Some(['o', 'a']) => Some((O, 1.1)),
            Some(['n', 'g']) | Some(['c', 'k']) => Some((Ee, 0.6)),
            _ => None,
        };
        if let Some((v, wt)) = two {
            push(v, wt);
            i += 2;
            continue;
        }
        // "ou"/"ow" and "ai"/"ay" glide from one shape to the next.
        match pair {
            Some(['o', 'u']) | Some(['o', 'w']) => {
                push(O, 0.8);
                push(U, 0.6);
                i += 2;
                continue;
            }
            Some(['a', 'i']) | Some(['a', 'y']) => {
                push(Ai, 0.8);
                push(Ee, 0.5);
                i += 2;
                continue;
            }
            _ => {}
        }
        // A silent final e ("make", "love").
        if c == 'e' && i == n - 1 && n > 2 && !matches!(w[i - 1], 'a' | 'e' | 'i' | 'o' | 'u') {
            break;
        }
        let one = match c {
            'a' => (Ai, 1.0),
            'e' => (Eh, 0.9),
            'i' => (Eh, 0.8),
            'o' => (O, 1.0),
            'u' => (Eh, 0.8),
            'y' => (Ee, 0.8),
            'w' => (U, 0.7),
            'm' | 'b' | 'p' => (Mbp, 0.8),
            'f' | 'v' => (Fv, 0.8),
            'l' => (L, 0.7),
            'r' => (U, 0.5),
            '0'..='9' => (Eh, 1.6),
            _ => (Ee, 0.6),
        };
        push(one.0, one.1);
        i += 1;
    }
    if out.is_empty() {
        out.push((Eh, 1.0));
    }
    out
}

/// Drop runs shorter than two frames (except closed lips, which must show
/// even when quick), so the mouth doesn't flicker.
fn smooth(v: &mut [Viseme]) {
    for i in 1..v.len().saturating_sub(1) {
        let (a, b, c) = (v[i - 1], v[i], v[i + 1]);
        if b != a && b != c && b != Viseme::Mbp {
            v[i] = a;
        }
    }
}

/// Speech loudness at a media frame, a little smoothed.
fn level(env: &[f32], i: usize) -> f32 {
    let v = env.get(i).copied().unwrap_or(0.0);
    let prev = i.checked_sub(1).and_then(|j| env.get(j)).copied().unwrap_or(v);
    v * 0.7 + prev * 0.3
}

/// One mouth shape per frame of `media`, from its words (if any) and envelope.
pub fn timeline(text: Option<&str>, media: &Media) -> Vec<Viseme> {
    let n = (media.duration_frames as usize).max(media.envelope.len());
    let env = &media.envelope;
    let peak = env.iter().cloned().fold(0.0f32, f32::max).max(1e-3);
    let mut out = vec![Viseme::Rest; n];
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            for word in captions::time_words(t, env, media.duration_frames) {
                let parts = sounds(&word.text);
                let total: f32 = parts.iter().map(|p| p.1).sum();
                let (s, e) = (word.start, word.end.max(word.start + 1.0));
                let f0 = s.floor().max(0.0) as usize;
                let f1 = (e.ceil() as usize).min(n);
                for (f, slot) in out.iter_mut().enumerate().take(f1).skip(f0) {
                    // Where this frame's middle falls in the word.
                    let pos = ((f as f32 + 0.5 - s) / (e - s)).clamp(0.0, 0.999) * total;
                    let mut acc = 0.0;
                    let mut v = parts[0].0;
                    for &(pv, w) in &parts {
                        v = pv;
                        acc += w;
                        if pos < acc {
                            break;
                        }
                    }
                    // Quiet vowels open less; loud ones open wide.
                    let l = level(env, f) / peak;
                    if v.is_vowel() && l < 0.12 {
                        v = Viseme::Ee;
                    } else if v == Viseme::Ai && l < 0.35 {
                        v = Viseme::Eh;
                    } else if v == Viseme::Eh && l > 0.85 {
                        v = Viseme::Ai;
                    }
                    *slot = v;
                }
            }
        }
        None => {
            for (f, slot) in out.iter_mut().enumerate() {
                let l = level(env, f) / peak;
                let prev = level(env, f.saturating_sub(1)) / peak;
                let next = level(env, f + 1) / peak;
                // A dip between two syllables closes the lips for a moment.
                let dip = l < prev - 0.08 && l <= next && l < 0.45;
                // Vary the open shapes a little, the same way every time.
                let pick = hash_str(&format!("{}:{}", media.id, f / 4)) % 3;
                *slot = if l < 0.1 {
                    Viseme::Rest
                } else if dip {
                    Viseme::Mbp
                } else if l < 0.3 {
                    Viseme::Ee
                } else if l < 0.6 {
                    [Viseme::Eh, Viseme::O, Viseme::Eh][pick as usize]
                } else {
                    [Viseme::Ai, Viseme::Ai, Viseme::O][pick as usize]
                };
            }
        }
    }
    smooth(&mut out);
    out
}

/// Words a clip says, for lip sync: its subtitle text or typed line.
pub fn clip_text<'a>(clip: &'a AudioClip, media: &'a Media) -> Option<&'a str> {
    clip.caption_text(media)
}

/// The simple open/close mouth for a loudness level (0..1).
pub fn simple(level: f32) -> Viseme {
    match level {
        v if v < 0.12 => Viseme::Rest,
        v if v < 0.35 => Viseme::Ee,
        v if v < 0.65 => Viseme::Eh,
        _ => Viseme::Ai,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Viseme::*;

    #[test]
    fn words_become_shapes() {
        let s: Vec<Viseme> = sounds("map").into_iter().map(|p| p.0).collect();
        assert_eq!(s, vec![Mbp, Ai, Mbp]);
        let s: Vec<Viseme> = sounds("food").into_iter().map(|p| p.0).collect();
        assert_eq!(s, vec![Fv, U, Ee]);
        let s: Vec<Viseme> = sounds("love").into_iter().map(|p| p.0).collect();
        assert_eq!(s, vec![L, O, Fv], "silent e dropped");
    }

    #[test]
    fn timeline_closes_lips_and_rests_in_pauses() {
        let mut env = vec![0.0; 5];
        env.extend(std::iter::repeat_n(0.8, 30));
        env.extend(std::iter::repeat_n(0.0, 10));
        let m = Media { id: "m".into(), duration_frames: env.len() as u32, envelope: env, ..Default::default() };
        let t = timeline(Some("Mama made pie"), &m);
        assert_eq!(t.len(), 45);
        assert_eq!(t[0], Rest, "silent lead-in");
        assert_eq!(t[44], Rest, "silent tail");
        assert!(t[5..35].contains(&Mbp));
        assert!(t[5..35].iter().any(|v| matches!(v, Ai | Eh)));
        // Without words: shapes from the loudness alone.
        let t = timeline(None, &m);
        assert_eq!(t[0], Rest);
        assert!(t[10..30].iter().all(|v| *v != Rest));
    }
}
