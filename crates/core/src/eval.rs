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

/// Output pixels per world unit at zoom 1. The world is always laid out in
/// design units (1280x720); any frame shape shows the part of it that covers
/// the frame, like cropping: 1080p scales up 1.5x, vertical 9:16 shows a
/// full-height slice, square a 720x720 slice.
pub fn view_scale(project: &Project) -> f32 {
    (project.width as f32 / DESIGN_W).max(project.height as f32 / DESIGN_H)
}

/// Camera transform for a scene at a local frame.
pub fn camera_transform(project: &Project, scene: &Scene, local: u32) -> Affine {
    let k = camera_at(scene, local);
    let (w, h) = (project.width as f32, project.height as f32);
    let z = k.zoom.max(0.05) * view_scale(project);
    Affine::translate(w / 2.0, h / 2.0).then(&Affine::scale(z, z)).then(&Affine::translate(-k.x, -k.y))
}

fn scene_ops(project: &Project, lib: &Library, scene: &Scene, local: u32) -> (Vec<Op>, Vec<ItemHit>) {
    let mut world = Vec::new();

    // Background, in design units like everything else in the world.
    if let Some(bg) = lib.backgrounds.get(&scene.background.preset) {
        world.push(Op::group(Affine::IDENTITY, 1.0, bg.ops(&scene.background.colors)));
    } else {
        let c = scene.background.colors.first().copied().unwrap_or(Color::rgb(0xee, 0xef, 0xf4));
        world.push(Op::fill(Path::rect(-DESIGN_W, -DESIGN_H, DESIGN_W * 3.0, DESIGN_H * 3.0), c));
    }

    let cam = camera_transform(project, scene, local);
    // Pinned text ignores the camera: it is placed as if the camera were at
    // its home position, and drawn above the world.
    let pin = pinned_transform(project);
    let mut pinned = Vec::new();
    let mut hits = Vec::with_capacity(scene.items.len());
    for item in &scene.items {
        let is_pinned = is_pinned(item);
        let cam = if is_pinned { pin } else { cam };
        match item_op(project, lib, scene, item, local) {
            Some(op) => {
                let b = op.bounds();
                hits.push(ItemHit { id: item.id.clone(), bounds: if b.is_empty() { b } else { b.transformed(&cam) }, visible: true });
                if is_pinned {
                    pinned.push(op);
                } else {
                    world.push(op);
                }
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
    let mut ops = vec![Op::group(cam, 1.0, world)];
    if !pinned.is_empty() {
        ops.push(Op::group(pin, 1.0, pinned));
    }
    ops.extend(crate::captions::caption_ops(project, scene, local));
    (ops, hits)
}

/// Text pinned to the frame (it does not follow the camera).
pub fn is_pinned(item: &Item) -> bool {
    matches!(&item.kind, ItemKind::Text(t) if t.pinned)
}

/// Where pinned items are drawn: the camera at its home position (centre of
/// the design stage, zoom 1), whatever the scene's camera does.
pub fn pinned_transform(project: &Project) -> Affine {
    let (w, h) = (project.width as f32, project.height as f32);
    let z = view_scale(project);
    let home = CamKey::default();
    Affine::translate(w / 2.0, h / 2.0).then(&Affine::scale(z, z)).then(&Affine::translate(-home.x, -home.y))
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
    let mut spin = 0.0;

    // Enter / exit effects
    let scene_end = item.end.unwrap_or(scene.duration.max(1));
    let to_end = scene_end as f32 - local as f32;
    let view_h = project.height as f32 / view_scale(project);
    let mut apply = |effect: Effect, t: f32, entering: bool| {
        if t >= 1.0 {
            return;
        }
        let t = t.max(0.0);
        let e = Ease::Out.apply(t);
        match effect {
            Effect::None => {}
            Effect::Fade => opacity *= t,
            Effect::Pop => scale *= if entering { Ease::Back.apply(t) } else { e },
            // Slide distances are in world units: most of the visible frame.
            Effect::SlideLeft => x -= (1.0 - e) * (project.width as f32 / view_scale(project) * 0.8),
            Effect::SlideRight => x += (1.0 - e) * (project.width as f32 / view_scale(project) * 0.8),
            Effect::SlideUp => y += (1.0 - e) * (view_h * 0.9),
            Effect::SlideDown => y -= (1.0 - e) * (view_h * 0.9),
            Effect::Bounce => {
                if entering {
                    y -= (1.0 - bounce_out(t)) * view_h * 0.6;
                    opacity *= (t * 4.0).min(1.0);
                } else {
                    // A small hop, then up and away.
                    y -= (1.0 - t) * (1.0 - t) * view_h * 0.7 - (1.0 - t) * 30.0;
                    opacity *= (t * 3.0).min(1.0);
                }
            }
            Effect::Zoom => {
                scale *= 1.0 + (1.0 - e) * 1.6;
                opacity *= t;
            }
            Effect::Spin => {
                scale *= e;
                spin += (1.0 - e) * if entering { -360.0 } else { 360.0 };
            }
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
            let speech = speech_mouth(project, scene, &item.id, c.lip_sync, local);
            let mut pose = character_pose(project, lib, item, c, local, secs, moving, dir, flip, speech.is_some());
            // A shape keyed by hand wins; from behind there is no mouth to move.
            let auto = if c.lip_sync == crate::lipsync::LipSync::Off { None } else { speech };
            let mouth = anim::mouth(&item.tracks, local).or(auto).unwrap_or_default();
            let mouth = if character_view(item, c, local, moving, dir, flip).yaw().abs() > 105.0 { crate::lipsync::Viseme::Rest } else { mouth };
            pose = crate::poses::apply_strips(pose, &c.poses, local, fps, project, lib);
            anim::apply_bones(&item.tracks, &mut pose, local);
            let phase = (hash_str(&item.id) % 1000) as f32 / 1000.0 * 3.0;
            let blink = ((local as f32 / fps) + phase).rem_euclid(3.6) < 0.12;
            let face = Face { expression: anim::expression(c, &item.tracks, local), blink, mouth };
            let look = anim::resolve_look(c, &item.tracks, local);
            let wear = crate::parts::worn(project, lib, &look);
            let view = character_view(item, c, local, moving, dir, flip);
            draw_character(&look, &pose, &face, &wear, view.yaw())
        }
        ItemKind::Prop(p) => match lib.props.get(&p.prop) {
            Some(d) => d.ops(&p.colors),
            None => vec![Op::fill(Path::round_rect(-40.0, -80.0, 80.0, 80.0, 8.0), Color::rgb(0xff, 0x00, 0xff))],
        },
        ItemKind::Text(t) => {
            let t = anim::resolve_text(t, &item.tracks, local);
            let until_end = item.end.map(|e| (e as f32 - local as f32) / fps);
            text_ops(&t, since / fps, until_end, &item.id)
        }
    };
    let sy = scale * tr.scale_y;
    let sx = (if flip { -scale } else { scale }) * tr.scale_x;
    let tf = Affine::translate(x, y).then(&Affine::rotate_deg(tr.rotation + spin)).then(&Affine::scale(sx, sy));
    // Text should never render mirrored.
    let tf = if matches!(item.kind, ItemKind::Text(_)) && flip {
        tf.then(&Affine::scale(-1.0, 1.0))
    } else {
        tf
    };
    Some(Op::group(tf, opacity, children))
}

/// The lip-sync mouth shape if this character is speaking at `local`
/// (`Some(Rest)` in a pause inside a line). With several lines at once the
/// loudest one wins.
fn speech_mouth(project: &Project, scene: &Scene, id: &str, mode: crate::lipsync::LipSync, local: u32) -> Option<crate::lipsync::Viseme> {
    use crate::lipsync::{self, LipSync};
    let mut best: Option<(f32, lipsync::Viseme)> = None;
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
        if best.is_some_and(|(b, _)| b >= v) {
            continue;
        }
        let shape = match mode {
            LipSync::Shapes => lipsync::timeline(lipsync::clip_text(clip, media), media).get(idx).copied().unwrap_or_default(),
            _ => lipsync::simple(v),
        };
        best = Some((v, shape));
    }
    best.map(|b| b.1)
}

/// Standard bounce-out easing (a ball dropped onto the floor).
fn bounce_out(t: f32) -> f32 {
    let (n, d) = (7.5625, 2.75);
    let t = t.clamp(0.0, 1.0);
    if t < 1.0 / d {
        n * t * t
    } else if t < 2.0 / d {
        let t = t - 1.5 / d;
        n * t * t + 0.75
    } else if t < 2.5 / d {
        let t = t - 2.25 / d;
        n * t * t + 0.9375
    } else {
        let t = t - 2.625 / d;
        n * t * t + 0.984375
    }
}

/// Seconds one letter (or word) takes to animate in a reveal.
fn reveal_secs(r: Reveal) -> f32 {
    match r {
        Reveal::None | Reveal::Typewriter => 0.0,
        Reveal::PopLetters => 0.28,
        Reveal::PopWords => 0.35,
        Reveal::RiseWords => 0.4,
        Reveal::Bounce => 0.6,
        Reveal::Fade => 0.35,
    }
}

/// Draws a text item. `secs` = seconds since it appeared; `until_end` =
/// seconds until it leaves (if it has an end), for leaving reveals.
fn text_ops(t: &TextItem, secs: f32, until_end: Option<f32>, id: &str) -> Vec<Op> {
    let size = t.size.max(4.0);
    let shown = if t.uppercase { t.text.to_uppercase() } else { t.text.clone() };
    let lay = text::layout_glyphs(&shown, t.font, size, t.width.max(40.0));
    let (bw, bh) = (lay.width, lay.height);
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
        TextStyle::Title | TextStyle::Plain => {}
    }

    // ---- reveals: progress 0..1 for every glyph
    // Older files: a typewriter speed without a reveal is a typewriter reveal.
    let (rin, speed_in) = if t.reveal == Reveal::None && t.typewriter > 0.0 { (Reveal::Typewriter, t.typewriter) } else { (t.reveal, t.reveal_speed) };
    let speed_in = speed_in.max(0.5);
    let speed_out = t.reveal_speed.max(0.5);
    let rout = if until_end.is_some() { t.reveal_out } else { Reveal::None };
    let units = |r: Reveal| if r.by_word() { lay.words } else { lay.letters }.max(1) as f32;
    let unit = |r: Reveal, g: &text::Glyph| if r.by_word() { g.word } else { g.index } as f32;
    // Seconds into leaving (negative = not leaving yet).
    let leaving = match (rout, until_end) {
        (Reveal::None, _) | (_, None) => f32::NEG_INFINITY,
        (Reveal::Typewriter, Some(te)) => units(rout) / speed_out - te,
        (r, Some(te)) => (units(r) - 1.0) / speed_out + reveal_secs(r) - te,
    };
    let typed_in = (secs * speed_in).floor();
    let typed_out = units(Reveal::Typewriter) - (leaving * speed_out).floor();
    // (style, progress) for a glyph.
    let phase = |g: &text::Glyph| -> (Reveal, f32) {
        if leaving >= 0.0 {
            let p = if rout == Reveal::Typewriter {
                if (g.index as f32) < typed_out {
                    1.0
                } else {
                    0.0
                }
            } else {
                1.0 - ((leaving - unit(rout, g) / speed_out) / reveal_secs(rout)).clamp(0.0, 1.0)
            };
            if p < 1.0 {
                return (rout, p);
            }
        }
        let p = match rin {
            Reveal::None => 1.0,
            Reveal::Typewriter => {
                if (g.index as f32) < typed_in {
                    1.0
                } else {
                    0.0
                }
            }
            r => ((secs - unit(r, g) / speed_in) / reveal_secs(r)).clamp(0.0, 1.0),
        };
        (rin, p)
    };

    // Word centres, for animating whole words.
    let mut word_box = vec![(f32::MAX, f32::MIN, 0.0f32); lay.words];
    for g in lay.glyphs.iter().filter(|g| !g.ch.is_whitespace()) {
        let b = &mut word_box[g.word];
        b.0 = b.0.min(g.x);
        b.1 = b.1.max(g.x + g.advance);
        b.2 = g.baseline;
    }
    let wave = t.emphasis == Emphasis::Wave;
    let mut glyphs: Vec<(Affine, f32, &Path)> = Vec::new();
    let mut cursor: Option<(f32, f32)> = None;
    let typing = (rin == Reveal::Typewriter && typed_in < units(Reveal::Typewriter)) || (rout == Reveal::Typewriter && leaving >= 0.0);
    for g in &lay.glyphs {
        let (r, p) = phase(g);
        if p <= 0.0 {
            continue;
        }
        if typing {
            cursor = Some((g.x + g.advance, g.baseline));
        }
        if g.path.is_empty() {
            continue;
        }
        let (cx, cy) = if r.by_word() {
            let b = word_box[g.word];
            ((b.0 + b.1) / 2.0, b.2 - size * 0.36)
        } else {
            g.center(size)
        };
        let around = |s: f32| Affine::translate(cx, cy).then(&Affine::scale(s, s)).then(&Affine::translate(-cx, -cy));
        let (mut tf, op) = match r {
            _ if p >= 1.0 => (Affine::IDENTITY, 1.0),
            Reveal::PopLetters | Reveal::PopWords => (around(Ease::Back.apply(p).max(0.0)), (p * 3.0).min(1.0)),
            Reveal::RiseWords => (Affine::translate(0.0, (1.0 - Ease::Out.apply(p)) * size * 0.9), p),
            Reveal::Bounce => (Affine::translate(0.0, -(1.0 - bounce_out(p)) * size * 2.2), (p * 4.0).min(1.0)),
            Reveal::Fade => (Affine::IDENTITY, p),
            Reveal::None | Reveal::Typewriter => (Affine::IDENTITY, 1.0),
        };
        if wave {
            let dy = (secs * 6.0 - g.index as f32 * 0.45).sin() * size * 0.12;
            tf = Affine::translate(0.0, dy).then(&tf);
        }
        glyphs.push((tf, op, &g.path));
    }
    if typing && cursor.is_none() {
        cursor = lay.glyphs.first().map(|g| (g.x, g.baseline));
    }

    let simple = glyphs.iter().all(|(a, o, _)| a.is_identity() && *o >= 1.0);
    let layer = |mk: &dyn Fn(Path) -> Op| -> Vec<Op> {
        if simple {
            let mut p = Path::new();
            for (_, _, g) in &glyphs {
                p.extend(g);
            }
            vec![mk(p)]
        } else {
            glyphs.iter().map(|(a, o, g)| Op::group(*a, *o, vec![mk((*g).clone())])).collect()
        }
    };
    if t.style == TextStyle::Title {
        let off = t.size * 0.08;
        out.push(Op::group(Affine::translate(0.0, off), 1.0, layer(&|p| Op::fill(p, Color::rgba(0, 0, 0, 90)))));
    }
    if t.stroke_width > 0.0 {
        let (c, w) = (t.stroke, t.stroke_width * 2.0);
        out.extend(layer(&|p| Op::stroke(p, c, w)));
    }
    let c = t.color;
    out.extend(layer(&|p| Op::fill(p, c)));
    if let Some((x, base)) = cursor {
        out.push(Op::fill(Path::rect(x + size * 0.04, base - lay.ascent * 0.8, (size * 0.07).max(2.0), lay.ascent * 0.8 + lay.descent * 0.35), c));
    }

    match t.emphasis {
        Emphasis::Pulse => {
            let s = 1.0 + 0.06 * (secs * 5.0).sin();
            vec![Op::group(Affine::scale(s, s), 1.0, out)]
        }
        Emphasis::Shake => {
            let ph = (hash_str(id) % 628) as f32 / 100.0;
            let a = size * 0.05;
            let (dx, dy) = ((secs * 47.0 + ph).sin() * a, (secs * 59.0 + ph * 1.7).cos() * a);
            let r = (secs * 41.0 + ph).sin() * 1.5;
            vec![Op::group(Affine::translate(dx, dy).then(&Affine::rotate_deg(r)), 1.0, out)]
        }
        _ => out,
    }
}

// A sound to mix into the export.
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

/// Which way a character faces at a frame (in its own space, before the
/// item's flip): its keyed or set view, or, while walking with auto-turn on,
/// the way it is going (side views; front or back when walking toward or away).
fn character_view(item: &Item, c: &crate::model::CharacterItem, local: u32, moving: bool, dir: f32, flip: bool) -> crate::model::View {
    use crate::model::View;
    if moving && c.auto_walk && c.auto_turn {
        let screen = if dir > 0.0 {
            View::Right
        } else if dir < 0.0 {
            View::Left
        } else {
            let (_, y0) = anim::item_xy(item, local as f32);
            let (_, y1) = anim::item_xy(item, local as f32 + 1.0);
            if y1 >= y0 { View::Front } else { View::Back }
        };
        return if flip { screen.mirrored() } else { screen };
    }
    anim::view(c, &item.tracks, local)
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
        let yaw = character_view(item, c, local, moving, dir, flip).yaw();
        // Seen from the side or 3/4 the legs swing forward and back.
        if (45.0..=135.0).contains(&yaw.abs()) {
            return crate::rig::locomotion_side(travelled / cycle, run, stride);
        }
        return crate::rig::locomotion_stride(travelled / cycle, run, fwd, stride);
    }
    let action_id = if speaking && c.auto_talk && c.action == "idle" { "talk" } else { c.action.as_str() };
    // Walking or running on the spot: the same cycles, driven by time.
    if action_id == "walk" || action_id == "run" {
        let run = action_id == "run";
        let phase = secs / if run { 0.62 } else { 1.0 };
        if (45.0..=135.0).contains(&anim::view(c, &item.tracks, local).yaw().abs()) {
            return crate::rig::locomotion_side(phase, run, 1.0);
        }
        return crate::rig::locomotion(phase, run, facing);
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
    let speaking = speech_mouth(project, sc, &it.id, crate::lipsync::LipSync::Simple, local).is_some();
    let mut pose = character_pose(project, lib, it, c, local, since / fps * c.speed.max(0.05), tr.moving, tr.dir, tr.flip, speaking);
    pose = crate::poses::apply_strips(pose, &c.poses, local, fps, project, lib);
    anim::apply_bones(&it.tracks, &mut pose, local);
    Some(pose)
}
