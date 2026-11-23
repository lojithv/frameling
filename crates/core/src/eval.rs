//! The deterministic timeline evaluator.
//!
//! `evaluate(project, library, frame)` is a pure function: the same inputs
//! always give the same `DisplayList`. Time is an integer frame number, never
//! the wall clock, which is what makes export frame-exact and repeatable.

use crate::anim;
use crate::display::{DisplayList, Op, Path};
use crate::geom::{hash_str, lerp, Affine, Color, Ease, Rect};
use crate::model::*;
use crate::pack::Library;
use crate::rig::{draw_character, sample_action, Face};
use crate::text;
use serde::Serialize;

/// Length of enter/exit effects in frames.
const EFFECT_FRAMES: f32 = 12.0;
/// Asset packs are designed for this stage size; other sizes are scaled.
const DESIGN_W: f32 = 1280.0;
const DESIGN_H: f32 = 720.0;

/// Screen-space bounds of an item, for hit-testing and selection in the editor.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemHit {
    pub id: Id,
    pub bounds: Rect,
    pub visible: bool,
}

/// Evaluate the whole project at a global frame, including scene transitions.
pub fn evaluate(project: &Project, lib: &Library, frame: u32) -> DisplayList {
    let mut dl = DisplayList { width: project.width, height: project.height, background: Some(Color::BLACK), ops: vec![] };
    let Some((i, local)) = project.locate(frame) else {
        return dl;
    };
    let scene = &project.scenes[i];
    let (cur, _) = scene_ops(project, lib, scene, local);
    let tr = scene.transition;
    if i == 0 || tr.kind == TransitionKind::Cut || tr.duration == 0 || local >= tr.duration {
        dl.ops = cur;
        return dl;
    }
    let prev_scene = &project.scenes[i - 1];
    let (prev, _) = scene_ops(project, lib, prev_scene, prev_scene.duration.max(1) - 1 + local);
    let p = (local as f32 + 1.0) / (tr.duration as f32 + 1.0);
    let (w, h) = (project.width as f32, project.height as f32);
    let black = |a: f32| Op::fill(Path::rect(0.0, 0.0, w, h), Color::BLACK.with_alpha(a));
    dl.ops = match tr.kind {
        TransitionKind::Cut => cur,
        TransitionKind::Fade => vec![Op::group(Affine::IDENTITY, 1.0, prev), Op::group(Affine::IDENTITY, p, cur)],
        TransitionKind::FadeBlack => {
            if p < 0.5 {
                vec![Op::group(Affine::IDENTITY, 1.0, prev), black(p * 2.0)]
            } else {
                vec![Op::group(Affine::IDENTITY, 1.0, cur), black((1.0 - p) * 2.0)]
            }
        }
        TransitionKind::Slide => {
            let e = Ease::InOut.apply(p);
            vec![
                Op::group(Affine::translate(-e * w, 0.0), 1.0, prev),
                Op::group(Affine::translate((1.0 - e) * w, 0.0), 1.0, cur),
            ]
        }
        TransitionKind::Zoom => {
            let e = Ease::InOut.apply(p);
            let about = |s: f32| {
                Affine::translate(w / 2.0, h / 2.0).then(&Affine::scale(s, s)).then(&Affine::translate(-w / 2.0, -h / 2.0))
            };
            vec![
                Op::group(about(1.0 + 0.6 * e), 1.0, prev),
                Op::group(about(0.85 + 0.15 * e), e, cur),
            ]
        }
    };
    dl
}

/// Evaluate one scene on its own (what the editor shows while editing a scene).
pub fn evaluate_scene(project: &Project, lib: &Library, scene: usize, local: u32) -> (DisplayList, Vec<ItemHit>) {
    let mut dl = DisplayList { width: project.width, height: project.height, background: Some(Color::BLACK), ops: vec![] };
    let Some(s) = project.scenes.get(scene) else {
        return (dl, vec![]);
    };
    let (ops, hits) = scene_ops(project, lib, s, local);
    dl.ops = ops;
    (dl, hits)
}

/// Camera centre and zoom at a local frame: the scene's from/to move, with
/// any `camera.*` keyframe tracks on top.
pub fn camera_at(scene: &Scene, local: u32) -> CamKey {
    let cam = &scene.camera;
    let base = if cam.moving {
        let t = cam.ease.apply(local as f32 / (scene.duration.max(2) - 1) as f32);
        CamKey { x: lerp(cam.from.x, cam.to.x, t), y: lerp(cam.from.y, cam.to.y, t), zoom: lerp(cam.from.zoom, cam.to.zoom, t) }
    } else {
        cam.from
    };
    anim::camera(scene, base, local)
}

/// Camera transform for a scene at a local frame.
pub fn camera_transform(project: &Project, scene: &Scene, local: u32) -> Affine {
    let k = camera_at(scene, local);
    let (w, h) = (project.width as f32, project.height as f32);
    let z = k.zoom.max(0.05);
    Affine::translate(w / 2.0, h / 2.0).then(&Affine::scale(z, z)).then(&Affine::translate(-k.x, -k.y))
}

fn scene_ops(project: &Project, lib: &Library, scene: &Scene, local: u32) -> (Vec<Op>, Vec<ItemHit>) {
    let mut world = Vec::new();
    let (w, h) = (project.width as f32, project.height as f32);

    // Background
    if let Some(bg) = lib.backgrounds.get(&scene.background.preset) {
        world.push(Op::group(Affine::scale(w / DESIGN_W, h / DESIGN_H), 1.0, bg.ops(&scene.background.colors)));
    } else {
        let c = scene.background.colors.first().copied().unwrap_or(Color::rgb(0xee, 0xef, 0xf4));
        world.push(Op::fill(Path::rect(-w, -h, w * 3.0, h * 3.0), c));
    }

    let cam = camera_transform(project, scene, local);
    let mut hits = Vec::with_capacity(scene.items.len());
    for item in &scene.items {
        match item_op(project, lib, scene, item, local) {
            Some(op) => {
                let b = op.bounds();
                hits.push(ItemHit { id: item.id.clone(), bounds: if b.is_empty() { b } else { b.transformed(&cam) }, visible: true });
                world.push(op);
            }
            None => {
                // Still report where it would be, so hidden items can be selected.
                let mut ghost = item.clone();
                ghost.start = 0;
                ghost.end = None;
                ghost.enter = Effect::None;
                ghost.exit = Effect::None;
                let b = item_op(project, lib, scene, &ghost, local.clamp(item.start, item.end.unwrap_or(u32::MAX).saturating_sub(1)))
                    .map(|o| o.bounds())
                    .unwrap_or_else(Rect::empty);
                hits.push(ItemHit { id: item.id.clone(), bounds: if b.is_empty() { b } else { b.transformed(&cam) }, visible: false });
            }
        }
    }
    (vec![Op::group(cam, 1.0, world)], hits)
}

/// Where an item is at a local frame (motion path and keyframes), whether it
/// is moving, and which way (-1, 0, 1).
pub fn item_position(item: &Item, local: u32) -> (f32, f32, bool, f32) {
    let t = anim::item_transform(item, local);
    (t.x, t.y, t.moving, t.dir)
}

fn item_op(project: &Project, lib: &Library, scene: &Scene, item: &Item, local: u32) -> Option<Op> {
    let end = item.end.unwrap_or(u32::MAX);
    if local < item.start || local >= end {
        return None;
    }
    let fps = project.fps.max(1) as f32;
    let since = (local - item.start) as f32;
    let tr = anim::item_transform(item, local);
    let (mut x, mut y, moving, dir, flip) = (tr.x, tr.y, tr.moving, tr.dir, tr.flip);
    let mut opacity = tr.opacity.clamp(0.0, 1.0);
    let mut scale = tr.scale;

    // Enter / exit effects
    let scene_end = item.end.unwrap_or(scene.duration.max(1));
    let to_end = scene_end as f32 - local as f32;
    let mut apply = |effect: Effect, t: f32, entering: bool| {
        if t >= 1.0 {
            return;
        }
        let e = Ease::Out.apply(t.max(0.0));
        match effect {
            Effect::None => {}
            Effect::Fade => opacity *= t.max(0.0),
            Effect::Pop => scale *= if entering { Ease::Back.apply(t.max(0.0)) } else { e },
            Effect::SlideLeft => x -= (1.0 - e) * (project.width as f32 * 0.8),
            Effect::SlideRight => x += (1.0 - e) * (project.width as f32 * 0.8),
            Effect::SlideUp => y += (1.0 - e) * (project.height as f32 * 0.9),
        }
    };
    apply(item.enter, since / EFFECT_FRAMES, true);
    if item.exit != Effect::None {
        apply(item.exit, (to_end - 1.0) / EFFECT_FRAMES, false);
        if to_end <= 0.0 {
            return None;
        }
    }
    if opacity <= 0.0 || scale.abs() < 1e-4 {
        return Some(Op::group(Affine::IDENTITY, 0.0, vec![]));
    }

    let children = match &item.kind {
        ItemKind::Character(c) => {
            let secs = since / fps * c.speed.max(0.05);
            let level = speech_level(project, scene, &item.id, local);
            let mut pose = character_pose(project, lib, item, c, local, secs, moving, dir, flip, level.is_some());
            pose = crate::poses::apply_strips(pose, &c.poses, local, fps, project, lib);
            anim::apply_bones(&item.tracks, &mut pose, local);
            let phase = (hash_str(&item.id) % 1000) as f32 / 1000.0 * 3.0;
            let blink = ((local as f32 / fps) + phase).rem_euclid(3.6) < 0.12;
            let face = Face { expression: anim::expression(c, &item.tracks, local), blink, mouth: level.unwrap_or(0) };
            let look = anim::resolve_look(c, &item.tracks, local);
            let wear = crate::parts::worn(project, lib, &look);
            draw_character(&look, &pose, &face, &wear)
        }
        ItemKind::Prop(p) => match lib.props.get(&p.prop) {
            Some(d) => d.ops(&p.colors),
            None => vec![Op::fill(Path::round_rect(-40.0, -80.0, 80.0, 80.0, 8.0), Color::rgb(0xff, 0x00, 0xff))],
        },
        ItemKind::Text(t) => text_ops(t, since, fps),
    };
    let sy = scale * tr.scale_y;
    let sx = (if flip { -scale } else { scale }) * tr.scale_x;
    let tf = Affine::translate(x, y).then(&Affine::rotate_deg(tr.rotation)).then(&Affine::scale(sx, sy));
    // Text should never render mirrored.
    let tf = if matches!(item.kind, ItemKind::Text(_)) && flip {
        tf.then(&Affine::scale(-1.0, 1.0))
    } else {
        tf
    };
    Some(Op::group(tf, opacity, children))
}

/// Lip-sync mouth level (1..=3) if this character is speaking at `local`.
fn speech_level(project: &Project, scene: &Scene, id: &str, local: u32) -> Option<u8> {
    let mut best: Option<f32> = None;
    for clip in scene.audio.iter().filter(|c| c.speaker.as_deref() == Some(id)) {
        let Some(media) = project.media.get(&clip.media) else { continue };
        let len = clip.play_frames(media);
        if local < clip.start || local >= clip.start + len {
            continue;
        }
        let idx = (clip.trim_start + local - clip.start) as usize;
        let v = media.envelope.get(idx).copied().unwrap_or(0.0);
        let prev = idx.checked_sub(1).and_then(|i| media.envelope.get(i)).copied().unwrap_or(v);
        let v = v * 0.7 + prev * 0.3;
        best = Some(best.map_or(v, |b| b.max(v)));
    }
    best.map(|v| match v {
        v if v < 0.12 => 0,
        v if v < 0.35 => 1,
        v if v < 0.65 => 2,
        _ => 3,
    })
}

fn text_ops(t: &TextItem, since: f32, fps: f32) -> Vec<Op> {
    let visible = (t.typewriter > 0.0).then(|| (since / fps * t.typewriter).floor() as usize);
    let block = text::layout(&t.text, t.size.max(4.0), t.width.max(40.0), visible);
    let (bw, bh) = (block.width, block.height);
    let mut out = Vec::new();
    match t.style {
        TextStyle::Bubble | TextStyle::Thought => {
            let (px, py) = (t.size * 0.8, t.size * 0.6);
            let (x0, y0, w, h) = (-bw / 2.0 - px, -bh / 2.0 - py, bw + 2.0 * px, bh + 2.0 * py);
            let r = if t.style == TextStyle::Thought { h.min(w) / 2.0 } else { t.size * 0.8 };
            let body = Path::round_rect(x0, y0, w, h, r);
            let outline = Color::INK;
            let sw = 6.0;
            let sign = match t.tail {
                Tail::BottomLeft => Some(-1.0),
                Tail::BottomRight => Some(1.0),
                Tail::None => None,
            };
            let mut tail_shapes: Vec<Path> = Vec::new();
            if let Some(s) = sign {
                let base_x = s * (w / 2.0 - r.min(w / 3.0) - 10.0);
                let by = y0 + h;
                if t.style == TextStyle::Bubble {
                    tail_shapes.push(Path::polyline(
                        &[(base_x - 14.0, by - 4.0), (base_x + 14.0, by - 4.0), (base_x + s * 8.0 - s * 30.0 + s * 46.0, by + 34.0)],
                        true,
                    ));
                } else {
                    tail_shapes.push(Path::circle(base_x + s * 10.0, by + 14.0, 9.0));
                    tail_shapes.push(Path::circle(base_x + s * 24.0, by + 32.0, 5.5));
                }
            }
            for p in &tail_shapes {
                out.push(Op::stroke(p.clone(), outline, sw));
            }
            out.push(Op::stroke(body.clone(), outline, sw));
            for p in tail_shapes {
                out.push(Op::fill(p, t.fill));
            }
            out.push(Op::fill(body, t.fill));
        }
        TextStyle::Caption => {
            let (px, py) = (t.size * 0.7, t.size * 0.45);
            out.push(Op::fill(Path::round_rect(-bw / 2.0 - px, -bh / 2.0 - py, bw + 2.0 * px, bh + 2.0 * py, 10.0), t.fill));
        }
        TextStyle::Title => {
            let shadow = Op::group(Affine::translate(0.0, t.size * 0.08), 1.0, vec![Op::fill(block.path.clone(), Color::rgba(0, 0, 0, 90))]);
            out.push(shadow);
        }
        TextStyle::Plain => {}
    }
    out.push(Op::fill(block.path, t.color));
    out
}

/// A sound to mix into the export.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioEvent {
    pub media: Id,
    /// Global frame where playback starts.
    pub at: u32,
    /// Frames to skip into the media.
    pub trim: u32,
    /// Frames to play.
    pub frames: u32,
    pub volume: f32,
}

/// All audio on the global timeline, clipped to scene boundaries.
pub fn audio_events(project: &Project) -> Vec<AudioEvent> {
    let mut out = Vec::new();
    for (scene, start) in project.scenes.iter().zip(project.scene_starts()) {
        for clip in &scene.audio {
            let Some(media) = project.media.get(&clip.media) else { continue };
            if clip.start >= scene.duration {
                continue;
            }
            let frames = clip.play_frames(media).min(scene.duration - clip.start);
            if frames == 0 {
                continue;
            }
            out.push(AudioEvent { media: clip.media.clone(), at: start + clip.start, trim: clip.trim_start, frames, volume: clip.volume.max(0.0) });
        }
    }
    out
}

/// One item on its own, fitted into `width` x `height` on a transparent
/// canvas: library thumbnails, the character creator and agent previews.
/// `parts` are project parts the item may wear (pack parts not in `lib`).
pub fn item_preview(lib: &Library, parts: &std::collections::BTreeMap<String, crate::pack::PartDef>, item: &Item, width: u32, height: u32, frame: u32) -> DisplayList {
    let mut item = item.clone();
    item.x = 0.0;
    item.y = 0.0;
    item.scale = 1.0;
    item.start = 0;
    item.end = None;
    item.motion = None;
    item.enter = crate::model::Effect::None;
    item.exit = crate::model::Effect::None;
    item.tracks.retain(|p, _| !p.starts_with("transform."));
    if item.id.is_empty() {
        item.id = "preview".into();
    }
    let mut p = Project { parts: parts.clone(), ..Project::default() };
    let mut s = Scene::new("preview", "preview");
    s.background.preset = String::new();
    s.background.colors = vec![Color::rgba(0, 0, 0, 0)];
    s.items.push(item);
    p.scenes = vec![s];
    let (dl, hits) = evaluate_scene(&p, lib, 0, frame);
    let b = hits.first().map(|h| h.bounds).unwrap_or(Rect { x: -50.0, y: -100.0, w: 100.0, h: 100.0 });
    let pad = 0.08;
    let k = ((width as f32 * (1.0 - 2.0 * pad)) / b.w.max(1.0)).min((height as f32 * (1.0 - 2.0 * pad)) / b.h.max(1.0));
    let fit = Affine::translate(width as f32 / 2.0, height as f32 / 2.0)
        .then(&Affine::scale(k, k))
        .then(&Affine::translate(-(b.x + b.w / 2.0), -(b.y + b.h / 2.0)));
    // Drop the background op (first child of the camera group) for transparency.
    let mut ops = dl.ops;
    if let Some(Op::Group { children, .. }) = ops.first_mut() {
        if !children.is_empty() {
            children.remove(0);
        }
    }
    DisplayList { width, height, background: None, ops: vec![Op::group(fit, 1.0, ops)] }
}

/// Running starts above this speed (stage px per frame at scale 1).
const RUN_SPEED: f32 = 9.0;
/// Fastest cycles per frame before the stride stops growing and the feet
/// slide a little (a brisk walk, a sprint).
const MAX_WALK_CPF: f32 = 1.7 / 30.0;
const MAX_RUN_CPF: f32 = 2.7 / 30.0;

/// The fastest a character travels between its position keys (or along its
/// old-style movement), in stage px per frame. Constant for the item, so the
/// walk cycle never jumps when easing changes the speed.
fn travel_speed(item: &Item) -> f32 {
    let mut best: f32 = 0.0;
    let seg = |t: Option<&anim::Track>| -> Vec<(u32, f32)> { t.map(|t| t.keys.iter().map(|k| (k.frame, k.value.as_f64().unwrap_or(0.0) as f32)).collect()).unwrap_or_default() };
    for keys in [seg(item.tracks.get(anim::Channel::X.path())), seg(item.tracks.get(anim::Channel::Y.path()))] {
        for w in keys.windows(2) {
            let df = w[1].0.saturating_sub(w[0].0).max(1) as f32;
            best = best.max((w[1].1 - w[0].1).abs() / df);
        }
    }
    if let Some(m) = item.motion {
        let df = m.end.saturating_sub(m.start).max(1) as f32;
        best = best.max(((m.x - item.x).powi(2) + (m.y - item.y).powi(2)).sqrt() / df);
    }
    best
}

/// A character's animated pose before pose strips and bone keys: the walk or
/// run cycle while moving (locked to the distance travelled, so feet don't
/// slide), otherwise its action (talking while it speaks).
#[allow(clippy::too_many_arguments)]
fn character_pose(project: &Project, lib: &Library, item: &Item, c: &crate::model::CharacterItem, local: u32, secs: f32, moving: bool, dir: f32, flip: bool, speaking: bool) -> crate::rig::Pose {
    let facing = if flip { -1.0 } else { 1.0 };
    let stick_k = if c.look.style == crate::model::FigureStyle::Stick { 93.0 / 90.0 } else { 1.0 };
    if moving && c.auto_walk {
        let (x0, y0) = anim::item_xy(item, local as f32);
        let scale = anim::item_transform(item, local).scale.abs().max(0.05);
        // Speed in character units: decides walk or run and the stride.
        let speed = travel_speed(item) / scale;
        let run = speed > RUN_SPEED;
        let (base, max_cpf) = if run { (crate::rig::RUN_CYCLE, MAX_RUN_CPF) } else { (crate::rig::WALK_CYCLE, MAX_WALK_CPF) };
        // Longer steps when faster; beyond the longest step, the cadence caps
        // and the feet slide a little rather than spinning.
        let needed = speed / max_cpf;
        let stride = (needed / base).clamp(1.0, 1.6);
        let cycle = (base * stride).max(needed) * scale * stick_k;
        let travelled = if dir != 0.0 { x0 * dir } else { y0 };
        let fwd = if dir != 0.0 { dir * facing } else { facing };
        return crate::rig::locomotion_stride(travelled / cycle, run, fwd, stride);
    }
    let action_id = if speaking && c.auto_talk && c.action == "idle" { "talk" } else { c.action.as_str() };
    // Walking or running on the spot: the same cycles, driven by time.
    if action_id == "walk" || action_id == "run" {
        let run = action_id == "run";
        return crate::rig::locomotion(secs / if run { 0.62 } else { 1.0 }, run, facing);
    }
    project
        .actions
        .get(action_id)
        .or_else(|| lib.actions.get(action_id))
        .or_else(|| lib.actions.get("idle"))
        .map(|a| sample_action(a, secs))
        .unwrap_or_default()
}

/// A character's full pose at a scene frame (action or walk, pose strips and
/// bone keys), e.g. to save it as a reusable pose.
pub fn posed_character(project: &Project, lib: &Library, scene: &str, item: &str, local: u32) -> Option<crate::rig::Pose> {
    let sc = project.scene(scene)?;
    let it = sc.item(item)?;
    let ItemKind::Character(c) = &it.kind else { return None };
    let fps = project.fps.max(1) as f32;
    let since = local.saturating_sub(it.start) as f32;
    let tr = anim::item_transform(it, local);
    let speaking = speech_level(project, sc, &it.id, local).is_some();
    let mut pose = character_pose(project, lib, it, c, local, since / fps * c.speed.max(0.05), tr.moving, tr.dir, tr.flip, speaking);
    pose = crate::poses::apply_strips(pose, &c.poses, local, fps, project, lib);
    anim::apply_bones(&it.tracks, &mut pose, local);
    Some(pose)
}
