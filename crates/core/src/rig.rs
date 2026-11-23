//! Procedural character rig: poses, action sampling and drawing.
//!
//! A character is drawn front-facing from a `Look` (colours, hair, build...)
//! and a `Pose` (joint angles). Actions in asset packs are keyframed poses, so
//! new actions need no code. Origin is the point between the feet; y grows down.

use crate::display::{Op, Path};
use crate::geom::{Affine, Color};
use crate::model::{Accessory, BrowStyle, Build, EyeStyle, Expression, FacialHair, FigureStyle, HairStyle, Look, NoseStyle};
use crate::pack::{Anchor, Layer};
use crate::pack::ActionDef;
use serde::{Deserialize, Serialize};

/// Joint angles in degrees and offsets in pixels. Arm/leg angles are measured
/// from hanging straight down; positive raises the limb outward/upward on its
/// own side, negative swings it across the body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pose {
    /// Lift the whole character (jumps).
    pub rise: f32,
    /// Push the upper body down (breathing, steps).
    pub bob: f32,
    /// Rotate the upper body around the hips.
    pub lean: f32,
    /// Tilt the head.
    pub head: f32,
    /// Left (screen-left) shoulder and elbow.
    pub ls: f32,
    pub le: f32,
    /// Right shoulder and elbow.
    pub rs: f32,
    pub re: f32,
    /// Hips and knees.
    pub lh: f32,
    pub lk: f32,
    pub rh: f32,
    pub rk: f32,
    /// Horizontal eye direction, -1..1.
    pub look: f32,
    /// Right hand makes a thumbs-up, 0..1 (drawn above 0.5).
    pub thumb: f32,
}

macro_rules! pose_lerp {
    ($a:expr, $b:expr, $t:expr; $($f:ident),*) => {
        Pose { $($f: $a.$f + ($b.$f - $a.$f) * $t),* }
    };
}

impl Pose {
    pub fn lerp(&self, o: &Pose, t: f32) -> Pose {
        pose_lerp!(self, o, t; rise, bob, lean, head, ls, le, rs, re, lh, lk, rh, rk, look, thumb)
    }
}

/// Sample an action at `secs` seconds since it started.
pub fn sample_action(a: &ActionDef, secs: f32) -> Pose {
    let keys = &a.keys;
    if keys.len() == 1 || a.duration <= 0.0 {
        return keys[0].pose;
    }
    let mut u = secs / a.duration;
    if a.r#loop {
        u = u.rem_euclid(1.0);
    } else {
        u = u.clamp(0.0, 1.0);
    }
    // Find the surrounding keys (wrapping for loops).
    let next = keys.iter().position(|k| k.t > u);
    let (k0, k1, span_start, span_len) = match next {
        Some(0) => {
            if a.r#loop {
                let last = keys.last().unwrap();
                (last, &keys[0], last.t - 1.0, keys[0].t - (last.t - 1.0))
            } else {
                return keys[0].pose;
            }
        }
        Some(i) => (&keys[i - 1], &keys[i], keys[i - 1].t, keys[i].t - keys[i - 1].t),
        None => {
            if a.r#loop {
                let last = keys.last().unwrap();
                (last, &keys[0], last.t, keys[0].t + 1.0 - last.t)
            } else {
                return keys.last().unwrap().pose;
            }
        }
    };
    let t = if span_len <= 1e-6 { 1.0 } else { ((u - span_start) / span_len).clamp(0.0, 1.0) };
    let s = t * t * (3.0 - 2.0 * t); // smoothstep between keys
    k0.pose.lerp(&k1.pose, s)
}

/// Face state for one frame.
#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub expression: Expression,
    pub blink: bool,
    /// Mouth opening from lip sync, 0..=3 (0 = closed / expression mouth).
    pub mouth: u8,
}

/// A wearable part ready to draw: its ops (colours applied) and where it goes.
#[derive(Clone, Debug)]
pub struct Worn {
    pub anchor: Anchor,
    pub layer: Layer,
    pub hides_hair: bool,
    pub ops: Vec<Op>,
}

/// The worn parts for one anchor and layer, placed by `t`.
pub(crate) fn worn_at(wear: &[Worn], anchor: Anchor, layer: Layer, t: Affine) -> Option<Op> {
    let ops: Vec<Op> = wear.iter().filter(|w| w.anchor == anchor && w.layer == layer).flat_map(|w| w.ops.iter().cloned()).collect();
    (!ops.is_empty()).then(|| Op::group(t, 1.0, ops))
}

const INK: Color = Color::INK;
const MOUTH: Color = Color::rgb(0x5a, 0x1e, 0x2c);
const TONGUE: Color = Color::rgb(0xe8, 0x6f, 0x7e);

fn dir(angle_deg: f32, side: f32) -> (f32, f32) {
    let r = angle_deg.to_radians();
    (side * r.sin(), r.cos())
}

/// Two-segment limb drawn as round-capped strokes. Returns the end point.
fn limb(
    out: &mut Vec<Op>,
    start: (f32, f32),
    a1: f32,
    a2: f32,
    len1: f32,
    len2: f32,
    side: f32,
    c1: Color,
    c2: Color,
    width: f32,
) -> (f32, f32) {
    let d1 = dir(a1, side);
    let mid = (start.0 + d1.0 * len1, start.1 + d1.1 * len1);
    let d2 = dir(a1 + a2, side);
    let end = (mid.0 + d2.0 * len2, mid.1 + d2.1 * len2);
    if c1 == c2 {
        out.push(Op::stroke(Path::polyline(&[start, mid, end], false), c1, width));
    } else {
        out.push(Op::stroke(Path::polyline(&[mid, end], false), c2, width * 0.88));
        out.push(Op::stroke(Path::polyline(&[start, mid], false), c1, width));
    }
    end
}

/// Draw a character. Coordinates: feet at (0,0), about 330px tall.
pub fn draw_character(look: &Look, pose: &Pose, face: &Face, wear: &[Worn]) -> Vec<Op> {
    if look.style == FigureStyle::Stick {
        return crate::stick::draw_stick(look, pose, face, wear);
    }
    let mut out = Vec::new();
    let width: f32 = match look.build {
        Build::Slim => 74.0,
        Build::Regular => 88.0,
        Build::Broad => 108.0,
    };
    let half = width / 2.0;
    let hip_y = -100.0;
    let hip_x = (half - 22.0).max(14.0);

    // ---- legs (not affected by lean) -------------------------------------
    let mut legs = Vec::new();
    for (side, h, k) in [(-1.0f32, pose.lh, pose.lk), (1.0, pose.rh, pose.rk)] {
        let start = (side * hip_x, hip_y);
        let foot = limb(&mut legs, start, h, k, 46.0, 44.0, side, look.bottom, look.bottom, 25.0);
        legs.push(Op::fill(Path::ellipse(foot.0 + side * 7.0, foot.1 + 4.0, 18.0, 9.5), look.shoes));
    }

    // ---- upper body, rotated by lean around the hips -----------------------
    let mut upper = Vec::new();
    let top_y = -214.0;

    // Long hair goes behind everything else on the upper body.
    let head_c = (0.0f32, -272.0f32);
    let mut head_ops = Vec::new();
    let head_t = Affine::translate(head_c.0, head_c.1 - 0.0)
        .then(&Affine::rotate_deg(pose.head))
        .then(&Affine::translate(0.0, 0.0));

    let body_sx = width / 88.0;
    let neck_t = Affine::translate(0.0, top_y).then(&Affine::scale(body_sx, 1.0));
    let torso_t = Affine::translate(0.0, top_y + 40.0).then(&Affine::scale(body_sx, 1.0));
    // Back-layer parts (capes, backpacks, hat brims behind the head) first.
    for (a, t) in [(Anchor::Neck, neck_t), (Anchor::Torso, torso_t), (Anchor::Head, head_t)] {
        upper.extend(worn_at(wear, a, Layer::Back, t));
    }

    if look.hair_style == HairStyle::Long {
        upper.push(Op::group(
            head_t,
            1.0,
            vec![Op::fill(Path::round_rect(-60.0, -46.0, 120.0, 118.0, 44.0), look.hair.darken(0.08))],
        ));
    }

    // Torso: trousers underneath, shirt on top.
    upper.push(Op::fill(Path::round_rect(-half, -150.0, width, 62.0, 18.0), look.bottom));
    upper.push(Op::fill(Path::round_rect(-half, top_y, width, 104.0, 30.0), look.top));
    upper.push(Op::fill(Path::rect(-half, -150.0, width, 34.0), look.top));
    upper.push(Op::fill(Path::rect(-half, -118.0, width, 6.0), look.bottom.darken(0.25)));
    // Collar shading
    upper.push(Op::fill(Path::ellipse(0.0, top_y + 4.0, 20.0, 7.0), look.top.darken(0.18)));

    if look.accessory == Accessory::Tie {
        let mut p = Path::new();
        p.move_to(-7.0, top_y + 6.0)
            .line_to(7.0, top_y + 6.0)
            .line_to(10.0, top_y + 64.0)
            .line_to(0.0, top_y + 76.0)
            .line_to(-10.0, top_y + 64.0)
            .close();
        upper.push(Op::fill(p, look.accent));
        upper.push(Op::fill(Path::round_rect(-8.0, top_y + 2.0, 16.0, 12.0, 3.0), look.accent.darken(0.2)));
    }

    upper.extend(worn_at(wear, Anchor::Torso, Layer::Front, torso_t));

    // Arms
    let sh_y = top_y + 16.0;
    let sh_x = half - 9.0;
    for (side, s, e) in [(-1.0f32, pose.ls, pose.le), (1.0, pose.rs, pose.re)] {
        let hand = limb(&mut upper, (side * sh_x, sh_y), s, e, 42.0, 40.0, side, look.top, look.skin, 21.0);
        let anchor = if side > 0.0 { Anchor::HandR } else { Anchor::HandL };
        upper.extend(worn_at(wear, anchor, Layer::Back, Affine::translate(hand.0, hand.1)));
        upper.push(Op::fill(Path::circle(hand.0, hand.1, 11.0), look.skin));
        if side > 0.0 && pose.thumb > 0.5 {
            upper.push(Op::fill(Path::round_rect(hand.0 - 4.0, hand.1 - 25.0, 8.5, 20.0, 4.2), look.skin));
        }
        upper.extend(worn_at(wear, anchor, Layer::Front, Affine::translate(hand.0, hand.1)));
    }

    // Neck
    upper.push(Op::fill(Path::round_rect(-11.0, top_y - 16.0, 22.0, 22.0, 6.0), look.skin.darken(0.12)));
    upper.extend(worn_at(wear, Anchor::Neck, Layer::Front, neck_t));

    // ---- head -------------------------------------------------------------
    draw_head(&mut head_ops, look, pose, face, wear);
    head_ops.extend(worn_at(wear, Anchor::Head, Layer::Front, Affine::IDENTITY));
    upper.push(Op::group(head_t, 1.0, head_ops));

    let lean_t = Affine::translate(0.0, hip_y + pose.bob)
        .then(&Affine::rotate_deg(pose.lean))
        .then(&Affine::translate(0.0, -hip_y));

    out.push(Op::group(Affine::translate(0.0, -pose.rise), 1.0, {
        let mut v = legs;
        v.push(Op::group(lean_t, 1.0, upper));
        v
    }));
    out
}

fn draw_head(out: &mut Vec<Op>, look: &Look, pose: &Pose, face: &Face, wear: &[Worn]) {
    let skin = look.skin;
    let r = 50.0;
    // Bun sits behind the head
    if look.hair_style == HairStyle::Bun {
        out.push(Op::fill(Path::circle(0.0, -62.0, 21.0), look.hair));
    }
    // Ears + head
    for side in [-1.0f32, 1.0] {
        out.push(Op::fill(Path::circle(side * 48.0, 4.0, 10.0), skin.darken(0.06)));
    }
    out.push(Op::fill(Path::ellipse(0.0, 0.0, r, r - 2.0), skin));

    // Cheeks
    let blush = matches!(face.expression, Expression::Happy | Expression::Wink);
    if blush {
        for side in [-1.0f32, 1.0] {
            out.push(Op::fill(Path::ellipse(side * 29.0, 14.0, 9.0, 5.5), Color::rgba(0xf2, 0x7a, 0x86, 110)));
        }
    }

    draw_eyes(out, look, pose, face);
    draw_beard(out, look);
    draw_mouth(out, face);
    draw_moustache(out, look);
    draw_nose(out, look);

    if !wear.iter().any(|w| w.hides_hair) {
        draw_hair(out, look);
    }

    match look.accessory {
        Accessory::Glasses => {
            for side in [-1.0f32, 1.0] {
                out.push(Op::stroke(Path::round_rect(side * 18.0 - 14.0, -12.0, 28.0, 24.0, 9.0), INK, 3.0));
            }
            out.push(Op::stroke(Path::polyline(&[(-4.0, -2.0), (4.0, -2.0)], false), INK, 3.0));
        }
        Accessory::Cap => {
            let mut p = Path::new();
            p.move_to(-53.0, -14.0).cubic_to(-54.0, -66.0, 54.0, -66.0, 53.0, -14.0).close();
            out.push(Op::fill(p, look.accent));
            out.push(Op::fill(Path::round_rect(-6.0, -20.0, 76.0, 12.0, 6.0), look.accent.darken(0.25)));
            out.push(Op::fill(Path::circle(0.0, -55.0, 5.0), look.accent.darken(0.2)));
        }
        Accessory::Bow => {
            let (bx, by) = (30.0, -46.0);
            out.push(Op::fill(Path::polyline(&[(bx, by), (bx - 18.0, by - 12.0), (bx - 18.0, by + 12.0)], true), look.accent));
            out.push(Op::fill(Path::polyline(&[(bx, by), (bx + 18.0, by - 12.0), (bx + 18.0, by + 12.0)], true), look.accent));
            out.push(Op::fill(Path::circle(bx, by, 6.0), look.accent.darken(0.2)));
        }
        _ => {}
    }
}

fn draw_eyes(out: &mut Vec<Op>, look: &Look, pose: &Pose, face: &Face) {
    use Expression::*;
    let brow_c = look.hair.darken(0.2);
    let ex = 18.0;
    let ey = -2.0;
    let (mut px, mut py) = (pose.look * 3.0, 0.0);
    match face.expression {
        Sad | Worried => py = 2.5,
        Confused => px = -2.5,
        Thinking => {
            px = 3.5;
            py = -3.5
        }
        _ => {}
    }
    for side in [-1.0f32, 1.0] {
        let cx = side * ex;
        let closed_arc = |up: bool| {
            let mut p = Path::new();
            let dy = if up { -6.0 } else { 4.0 };
            p.move_to(cx - 8.0, ey + 1.0).quad_to(cx, ey + dy, cx + 8.0, ey + 1.0);
            Op::stroke(p, INK, 3.5)
        };
        let wink_closed = face.expression == Wink && side < 0.0;
        if face.expression == Happy || wink_closed {
            out.push(closed_arc(true));
        } else if face.blink {
            out.push(closed_arc(false));
        } else if look.eyes == EyeStyle::Dots && face.expression != Surprised {
            out.push(Op::fill(Path::ellipse(cx + px * 0.5, ey + py * 0.5, 4.8, 5.8), INK));
            out.push(Op::fill(Path::circle(cx + px * 0.5 + 1.4, ey + py * 0.5 - 1.8, 1.3), Color::WHITE));
        } else {
            let (rx, ry, pr) = match (face.expression, look.eyes) {
                (Surprised, _) => (10.5, 13.0, 4.5),
                (_, EyeStyle::Wide) => (10.0, 12.0, 5.5),
                (_, EyeStyle::Narrow) => (9.5, 6.5, 4.2),
                _ => (8.5, 10.5, 5.5),
            };
            out.push(Op::fill(Path::ellipse(cx, ey, rx, ry), Color::WHITE));
            let pyc = (py + 1.0).clamp(-ry + pr, ry - pr);
            out.push(Op::fill(Path::circle(cx + px, ey + pyc, pr), INK));
            out.push(Op::fill(Path::circle(cx + px + 1.8, ey + pyc - 2.5, 1.6), Color::WHITE));
            if look.eyes == EyeStyle::Sleepy && face.expression != Surprised {
                // Heavy lids: skin over the top half of the eye, with a lid line.
                let mut lid = Path::new();
                lid.move_to(cx - rx - 1.0, ey).cubic_to(cx - rx, ey - ry * 1.45, cx + rx, ey - ry * 1.45, cx + rx + 1.0, ey).close();
                out.push(Op::fill(lid, look.skin));
                out.push(Op::stroke(Path::polyline(&[(cx - rx - 1.0, ey), (cx + rx + 1.0, ey)], false), INK, 2.6));
            }
            if look.eyes == EyeStyle::Lashes {
                // Two short lashes flicking out from the outer top corner.
                for deg in [28.0f32, 55.0] {
                    let a = deg.to_radians();
                    let (dx, dy) = (a.cos() * side, -a.sin());
                    let (x0, y0) = (cx + dx * rx, ey + dy * ry);
                    out.push(Op::stroke(Path::polyline(&[(x0, y0), (x0 + dx * 6.5, y0 + dy * 5.0)], false), INK, 2.4));
                }
            }
        }

        // Brows: (inner y, outer y) relative offsets
        let (inner, outer) = match face.expression {
            Sad => (-22.0, -16.0),
            Angry => (-12.0, -20.0),
            Surprised => (-25.0, -24.0),
            Thinking if side > 0.0 => (-24.0, -23.0),
            Worried => (-25.0, -17.0),
            Confused if side > 0.0 => (-27.0, -25.0),
            Confused => (-17.0, -17.0),
            _ => (-18.0, -19.0),
        };
        let brow_w = match look.brows {
            BrowStyle::None => continue,
            BrowStyle::Thin => 2.6,
            BrowStyle::Thick => 7.5,
            _ => 4.5,
        };
        if look.brows == BrowStyle::Arched {
            let mut p = Path::new();
            p.move_to(side * 8.0, inner + 1.0).quad_to(side * 17.0, inner.min(outer) - 7.0, side * 28.0, outer + 2.0);
            out.push(Op::stroke(p, brow_c, 4.0));
        } else {
            out.push(Op::stroke(Path::polyline(&[(side * 9.0, inner), (side * 27.0, outer)], false), brow_c, brow_w));
        }
    }
}

fn draw_mouth(out: &mut Vec<Op>, face: &Face) {
    use Expression::*;
    let (mx, my) = (0.0, 26.0);
    if face.mouth > 0 {
        let l = face.mouth as f32;
        let (rx, ry) = (8.0 + 3.5 * l, 3.0 + 4.0 * l);
        out.push(Op::fill(Path::ellipse(mx, my, rx, ry), MOUTH));
        if face.mouth >= 2 {
            out.push(Op::fill(Path::ellipse(mx, my + ry * 0.45, rx * 0.55, ry * 0.4), TONGUE));
        }
        return;
    }
    match face.expression {
        Happy | Wink => {
            let mut p = Path::new();
            p.move_to(mx - 15.0, my - 4.0).quad_to(mx, my - 6.0, mx + 15.0, my - 4.0).quad_to(mx, my + 16.0, mx - 15.0, my - 4.0).close();
            out.push(Op::fill(p, MOUTH));
        }
        Sad => {
            let mut p = Path::new();
            p.move_to(mx - 11.0, my + 5.0).quad_to(mx, my - 5.0, mx + 11.0, my + 5.0);
            out.push(Op::stroke(p, MOUTH, 3.5));
        }
        Angry => {
            let mut p = Path::new();
            p.move_to(mx - 11.0, my + 3.0).quad_to(mx, my - 2.0, mx + 11.0, my + 3.0);
            out.push(Op::stroke(p, MOUTH, 4.0));
        }
        Surprised => out.push(Op::fill(Path::ellipse(mx, my + 2.0, 7.0, 9.0), MOUTH)),
        Thinking => out.push(Op::stroke(Path::polyline(&[(mx - 6.0, my + 1.0), (mx + 9.0, my - 2.0)], false), MOUTH, 3.5)),
        Worried | Confused => out.push(Op::stroke(wavy_mouth(mx, my + 2.0, 12.0), MOUTH, 3.5)),
        Neutral => {
            let mut p = Path::new();
            p.move_to(mx - 10.0, my - 1.0).quad_to(mx, my + 6.0, mx + 10.0, my - 1.0);
            out.push(Op::stroke(p, MOUTH, 3.5));
        }
    }
}

fn draw_hair(out: &mut Vec<Op>, look: &Look) {
    let c = look.hair;
    let cap = || {
        let mut p = Path::new();
        p.move_to(-52.0, 2.0)
            .cubic_to(-58.0, -40.0, -32.0, -60.0, 0.0, -60.0)
            .cubic_to(32.0, -60.0, 58.0, -40.0, 52.0, 2.0)
            .cubic_to(46.0, -18.0, 36.0, -30.0, 14.0, -34.0)
            .cubic_to(4.0, -24.0, -22.0, -22.0, -36.0, -30.0)
            .cubic_to(-44.0, -22.0, -50.0, -10.0, -52.0, 2.0)
            .close();
        p
    };
    match look.hair_style {
        HairStyle::Bald => {}
        HairStyle::Short | HairStyle::Bun => out.push(Op::fill(cap(), c)),
        HairStyle::Long => {
            out.push(Op::fill(cap(), c));
            for side in [-1.0f32, 1.0] {
                out.push(Op::fill(Path::round_rect(side * 52.0 - 9.0, -20.0, 18.0, 62.0, 9.0), c));
            }
        }
        HairStyle::Spiky => {
            let pts = [
                (-52.0, 4.0), (-58.0, -28.0), (-46.0, -40.0), (-46.0, -62.0), (-26.0, -54.0), (-16.0, -76.0),
                (0.0, -58.0), (16.0, -74.0), (26.0, -54.0), (46.0, -62.0), (44.0, -40.0), (58.0, -28.0),
                (52.0, 4.0), (40.0, -22.0), (20.0, -32.0), (0.0, -28.0), (-20.0, -32.0), (-40.0, -22.0),
            ];
            out.push(Op::fill(Path::polyline(&pts, true), c));
        }
        HairStyle::Curly => {
            out.push(Op::fill(cap(), c));
            for i in 0..9 {
                let a = (200.0 + i as f32 * 17.5f32).to_radians();
                out.push(Op::fill(Path::circle(a.cos() * 48.0, a.sin() * 46.0 - 2.0, 15.0), c));
            }
        }
    }
}

fn draw_nose(out: &mut Vec<Op>, look: &Look) {
    let c = look.skin.darken(0.14);
    match look.nose {
        NoseStyle::Button => out.push(Op::fill(Path::ellipse(0.0, 12.0, 4.5, 3.5), c)),
        NoseStyle::Dot => out.push(Op::fill(Path::circle(0.0, 12.0, 2.4), look.skin.darken(0.3))),
        NoseStyle::Round => {
            out.push(Op::fill(Path::circle(0.0, 10.0, 8.5), look.skin.darken(0.06)));
            out.push(Op::fill(Path::circle(-2.5, 7.0, 2.5), look.skin.lighten(0.25)));
        }
        NoseStyle::Long => {
            let mut p = Path::new();
            p.move_to(-1.0, -4.0).quad_to(7.0, 10.0, 7.0, 15.0).quad_to(3.0, 18.0, -3.0, 15.0);
            out.push(Op::stroke(p, look.skin.darken(0.22), 3.0));
        }
        NoseStyle::None => {}
    }
}

/// Beards sit under the mouth; moustaches on top of it.
fn draw_beard(out: &mut Vec<Op>, look: &Look) {
    let c = look.hair;
    match look.facial_hair {
        FacialHair::Stubble => {
            let mut p = Path::new();
            p.move_to(-46.0, 10.0).cubic_to(-42.0, 54.0, 42.0, 54.0, 46.0, 10.0).cubic_to(30.0, 24.0, -30.0, 24.0, -46.0, 10.0).close();
            out.push(Op::fill(p, c.with_alpha(0.2)));
        }
        FacialHair::Goatee => out.push(Op::fill(Path::round_rect(-10.0, 33.0, 20.0, 18.0, 8.0), c)),
        FacialHair::Beard | FacialHair::FullBeard => {
            let (drop, top) = if look.facial_hair == FacialHair::FullBeard { (74.0, -6.0) } else { (58.0, 6.0) };
            let mut p = Path::new();
            p.move_to(-50.0, top)
                .cubic_to(-50.0, drop - 8.0, -22.0, drop, 0.0, drop)
                .cubic_to(22.0, drop, 50.0, drop - 8.0, 50.0, top)
                .cubic_to(42.0, 26.0, 26.0, 18.0, 0.0, 18.0)
                .cubic_to(-26.0, 18.0, -42.0, 26.0, -50.0, top)
                .close();
            out.push(Op::fill(p, c));
        }
        _ => {}
    }
}

fn draw_moustache(out: &mut Vec<Op>, look: &Look) {
    if !matches!(look.facial_hair, FacialHair::Moustache | FacialHair::Goatee | FacialHair::Beard | FacialHair::FullBeard) {
        return;
    }
    let c = look.hair;
    for side in [-1.0f32, 1.0] {
        let mut p = Path::new();
        p.move_to(0.0, 17.0)
            .cubic_to(side * 8.0, 13.0, side * 18.0, 14.0, side * 21.0, 22.0)
            .cubic_to(side * 14.0, 21.0, side * 7.0, 23.0, 0.0, 22.0)
            .close();
        out.push(Op::fill(p, c));
    }
}

/// A wobbly line mouth (worried / confused).
pub(crate) fn wavy_mouth(cx: f32, cy: f32, half: f32) -> Path {
    let mut p = Path::new();
    let q = half / 2.0;
    p.move_to(cx - half, cy).quad_to(cx - q * 1.5, cy - 4.0, cx - q, cy).quad_to(cx - q * 0.5, cy + 4.0, cx, cy).quad_to(cx + q * 0.5, cy - 4.0, cx + q, cy).quad_to(cx + q * 1.5, cy + 4.0, cx + half, cy);
    p
}
