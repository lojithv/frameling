//! Camera moves: one-click presets that write camera keyframes, and an auto
//! camera that follows whoever is talking.
//!
//! Everything here only writes keys on the scene's `camera.x`, `camera.y`,
//! `camera.zoom` and `camera.shake` tracks, so every move stays editable on
//! the timeline and renders the same in preview and export.

use crate::anim::{Interp, Key, Track};
use crate::eval::{camera_at, view_scale};
use crate::model::{ItemKind, Project, Scene};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A camera preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Move {
    /// Quick zoom in, ease back out (an emphasis beat).
    Punch,
    /// Shake for a moment (impacts, surprise).
    Shake,
    /// Slow zoom in.
    PushIn,
    /// Slow zoom out.
    PullOut,
    /// Smooth pan (and zoom) to the target.
    PanTo,
    /// Instant cut to a close-up of the target.
    CutTo,
    /// Very fast pan to the target with a little shake.
    WhipTo,
    /// Instant cut back to the wide shot.
    Reset,
}

/// How the auto camera covers a conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoStyle {
    /// Cut to a close-up of each speaker as their line starts.
    Cuts,
    /// Stay wide; punch in at each line and on exclamations.
    Punches,
}

/// Paths the camera tools write.
pub const PATHS: [&str; 4] = ["camera.x", "camera.y", "camera.zoom", "camera.shake"];

fn key(frame: u32, v: f32, interp: Interp) -> Key {
    Key { frame, value: Value::from((v * 1000.0).round() as f64 / 1000.0), interp }
}

fn put(scene: &mut Scene, path: &str, keys: Vec<Key>) {
    scene.tracks.entry(path.to_string()).or_insert_with(Track::default).upsert(keys);
}

/// Where the camera looks to frame an item: its upper body.
fn framing(project: &Project, scene: &Scene, id: &str, frame: u32, zoom: f32) -> Option<(f32, f32)> {
    let it = scene.item(id)?;
    let tr = crate::anim::item_transform(it, frame);
    let s = tr.scale.abs();
    let y = match it.kind {
        ItemKind::Character(_) => tr.y - 150.0 * s,
        _ => tr.y - 60.0 * s,
    };
    Some(clamp(project, tr.x, y, zoom))
}

/// Keep the view inside the 1280x720 world (backgrounds end there).
fn clamp(project: &Project, x: f32, y: f32, zoom: f32) -> (f32, f32) {
    let k = view_scale(project) * zoom.max(0.05);
    let (hw, hh) = (project.width as f32 / k / 2.0, project.height as f32 / k / 2.0);
    let cx = if hw * 2.0 >= 1280.0 { 640.0 } else { x.clamp(hw, 1280.0 - hw) };
    let cy = if hh * 2.0 >= 720.0 { 360.0 } else { y.clamp(hh, 720.0 - hh) };
    (cx, cy)
}

/// How far a close-up zooms: wide frames zoom in more, a narrow vertical
/// slice is already close.
fn closeup(project: &Project, strength: f32) -> f32 {
    let tall = project.height > project.width;
    (if tall { 1.18 } else { 1.45 }) * (1.0 + 0.25 * (strength - 1.0))
}

/// Write a camera preset into a scene at `frame`, lasting `duration` frames.
pub fn apply(project: &Project, scene: &mut Scene, mv: Move, frame: u32, duration: u32, target: Option<&str>, strength: f32) -> Result<(), String> {
    let s = strength.clamp(0.1, 3.0);
    let last = scene.duration.max(1) - 1;
    let f = frame.min(last);
    let d = duration.max(1);
    let end = (f + d).min(last);
    let cur = camera_at(scene, f);
    let before = f.saturating_sub(1);
    match mv {
        Move::Punch => {
            let z = cur.zoom * (1.0 + 0.16 * s);
            let mut keys = vec![key(f, z, Interp::EaseOut), key(end.max(f + 1).min(last.max(f + 1)), cur.zoom, Interp::Linear)];
            if f > 0 {
                keys.insert(0, key(before, cur.zoom, Interp::Constant));
            }
            // Punch towards the same point, so the frame doesn't drift.
            put(scene, "camera.x", vec![key(f, cur.x, Interp::Linear)]);
            put(scene, "camera.y", vec![key(f, cur.y, Interp::Linear)]);
            put(scene, "camera.zoom", keys);
        }
        Move::Shake => {
            let mut keys = vec![key(f, 14.0 * s, Interp::EaseOut), key(end.max(f + 1), 0.0, Interp::Linear)];
            if f > 0 {
                keys.insert(0, key(before, 0.0, Interp::Constant));
            }
            put(scene, "camera.shake", keys);
        }
        Move::PushIn | Move::PullOut => {
            let z1 = if mv == Move::PushIn { cur.zoom * (1.0 + 0.22 * s) } else { (cur.zoom / (1.0 + 0.22 * s)).max(1.0) };
            let (x1, y1) = match target.and_then(|t| framing(project, scene, t, end, z1)) {
                Some(p) => p,
                None => clamp(project, cur.x, cur.y, z1),
            };
            put(scene, "camera.zoom", vec![key(f, cur.zoom, Interp::EaseInOut), key(end, z1, Interp::Linear)]);
            put(scene, "camera.x", vec![key(f, cur.x, Interp::EaseInOut), key(end, x1, Interp::Linear)]);
            put(scene, "camera.y", vec![key(f, cur.y, Interp::EaseInOut), key(end, y1, Interp::Linear)]);
        }
        Move::PanTo | Move::WhipTo => {
            let t = target.ok_or("pick who the camera should move to")?;
            let z1 = if cur.zoom > 1.01 { cur.zoom } else { closeup(project, s) };
            let end = if mv == Move::WhipTo { (f + 6).min(last) } else { end };
            let (x1, y1) = framing(project, scene, t, end, z1).ok_or_else(|| format!("no item {t:?}"))?;
            let ease = if mv == Move::WhipTo { Interp::EaseIn } else { Interp::EaseInOut };
            put(scene, "camera.zoom", vec![key(f, cur.zoom, ease), key(end, z1, Interp::Linear)]);
            put(scene, "camera.x", vec![key(f, cur.x, ease), key(end, x1, Interp::Linear)]);
            put(scene, "camera.y", vec![key(f, cur.y, ease), key(end, y1, Interp::Linear)]);
            if mv == Move::WhipTo {
                put(scene, "camera.shake", vec![key(end, 8.0 * s, Interp::EaseOut), key((end + 8).min(last), 0.0, Interp::Linear)]);
            }
        }
        Move::CutTo => {
            let t = target.ok_or("pick who the camera should cut to")?;
            let z1 = closeup(project, s);
            let (x1, y1) = framing(project, scene, t, f, z1).ok_or_else(|| format!("no item {t:?}"))?;
            cut(scene, f, cur, (x1, y1, z1));
        }
        Move::Reset => {
            let home = scene.camera.from;
            cut(scene, f, cur, (home.x, home.y, home.zoom));
        }
    }
    crate::anim::normalize_tracks(&mut scene.tracks);
    Ok(())
}

/// A hard cut at `f`: hold the old view until `f - 1`, jump at `f`.
fn cut(scene: &mut Scene, f: u32, cur: crate::model::CamKey, to: (f32, f32, f32)) {
    for (path, a, b) in [("camera.x", cur.x, to.0), ("camera.y", cur.y, to.1), ("camera.zoom", cur.zoom, to.2)] {
        let mut keys = vec![key(f, b, Interp::Constant)];
        if f > 0 {
            keys.insert(0, key(f - 1, a, Interp::Constant));
        }
        put(scene, path, keys);
    }
}

/// Replace a scene's camera keys with an automatic edit of its dialogue.
pub fn auto(project: &Project, scene: &mut Scene, style: AutoStyle, strength: f32) -> Result<usize, String> {
    for p in PATHS {
        scene.tracks.remove(p);
    }
    let fps = project.fps.max(1);
    let mut lines: Vec<(u32, u32, Option<String>, Vec<u32>)> = vec![];
    for clip in &scene.audio {
        let Some(media) = project.media.get(&clip.media) else { continue };
        if clip.start >= scene.duration {
            continue;
        }
        let len = clip.play_frames(media).min(scene.duration - clip.start);
        // Exclamations get their own punch.
        let bangs = crate::captions::clip_words(clip, media)
            .iter()
            .filter(|w| w.text.contains('!'))
            .map(|w| clip.start + (w.start as u32).saturating_sub(clip.trim_start))
            .filter(|&f| f > clip.start + fps / 3 && f < scene.duration)
            .collect();
        lines.push((clip.start, clip.start + len, clip.speaker.clone(), bangs));
    }
    lines.sort_by_key(|l| l.0);
    let lines: Vec<_> = lines.into_iter().filter(|l| style == AutoStyle::Punches || l.2.is_some()).collect();
    if lines.is_empty() {
        return Err("this scene has no voice lines to follow".into());
    }
    let home = scene.camera.from;
    let mut moves = 0;
    match style {
        AutoStyle::Cuts => {
            let mut prev: Option<String> = None;
            let mut prev_end = 0;
            for (i, (start, end, speaker, _)) in lines.iter().enumerate() {
                let who = speaker.clone().unwrap();
                // Cut back to wide in a long pause, and before the very first line.
                if i > 0 && *start > prev_end + fps * 3 / 2 {
                    apply(project, scene, Move::Reset, prev_end + fps / 3, 1, None, strength)?;
                    prev = None;
                    moves += 1;
                }
                if prev.as_deref() != Some(who.as_str()) {
                    // Cut a moment before the line, so the face is on screen as it speaks.
                    let at = if i == 0 && *start < fps / 2 { 0 } else { start.saturating_sub(3) };
                    apply(project, scene, Move::CutTo, at, 1, Some(&who), strength)?;
                    moves += 1;
                }
                prev = Some(who);
                prev_end = *end;
            }
            if prev_end + fps < scene.duration {
                apply(project, scene, Move::Reset, prev_end + fps / 3, 1, None, strength)?;
                moves += 1;
            }
        }
        AutoStyle::Punches => {
            put(scene, "camera.x", vec![key(0, home.x, Interp::Linear)]);
            put(scene, "camera.y", vec![key(0, home.y, Interp::Linear)]);
            for (start, _, _, bangs) in &lines {
                apply(project, scene, Move::Punch, *start, fps * 2 / 3, None, strength)?;
                moves += 1;
                for &b in bangs {
                    apply(project, scene, Move::Punch, b, fps / 2, None, strength * 1.2)?;
                    moves += 1;
                }
            }
        }
    }
    Ok(moves)
}

/// Camera shake offset (world px) and roll (degrees) for an amplitude at a
/// frame: a few hashed sines, so it looks random but is the same every time.
pub fn shake(amp: f32, frame: u32, seed: u32) -> (f32, f32, f32) {
    if amp <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let t = frame as f32;
    let ph = (seed % 997) as f32 * 0.37;
    let x = (t * 1.9 + ph).sin() * 0.6 + (t * 3.7 + ph * 1.3).sin() * 0.4;
    let y = (t * 2.3 + ph * 0.7).cos() * 0.6 + (t * 4.1 + ph * 0.4).sin() * 0.4;
    let r = (t * 1.3 + ph * 2.1).sin();
    (x * amp, y * amp, r * amp * 0.08)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn punch_returns_to_the_old_zoom() {
        let p = Project::default();
        let mut s = Scene::new("s", "s");
        apply(&p, &mut s, Move::Punch, 30, 20, None, 1.0).unwrap();
        assert!((camera_at(&s, 29).zoom - 1.0).abs() < 1e-3);
        assert!(camera_at(&s, 30).zoom > 1.1);
        assert!((camera_at(&s, 50).zoom - 1.0).abs() < 1e-3);
    }

    #[test]
    fn cut_to_holds_then_jumps() {
        let mut p = Project::default();
        let mut s = Scene::new("s", "s");
        s.items.push(Item { id: "a".into(), x: 300.0, y: 640.0, kind: ItemKind::Character(Default::default()), ..Default::default() });
        p.scenes = vec![s.clone()];
        apply(&p, &mut s, Move::CutTo, 40, 1, Some("a"), 1.0).unwrap();
        assert_eq!(camera_at(&s, 39).x, 640.0);
        let k = camera_at(&s, 40);
        assert!(k.zoom > 1.3 && k.x < 500.0, "{k:?}");
    }
}
