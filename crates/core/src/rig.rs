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
    /// Lower the hips (px); the legs bend to stay where they are put.
    pub drop: f32,
    /// Foreshorten the thighs, 0..0.9 (thighs pointing at the camera when sitting).
    pub fold: f32,
    /// Foreshorten the shins, 0..0.9 (kneeling, shins pointing back).
    pub kneel: f32,
    /// Rotate the whole body around the hips, degrees clockwise (lying down).
    pub tilt: f32,
    /// 0..1: limb angles mean forward/back instead of out to the side
    /// (walk cycles), so they read correctly in side and 3/4 views.
    pub sagittal: f32,
    /// 0..1: lift or lower the body so its lowest point rests on the ground
    /// (sitting on the floor, kneeling, lying). Added to `rise`.
    pub ground: f32,
}

/// Lowest point (y + radius) of circles placed by `t`: for the ground solver.
pub(crate) fn lowest(points: &[(f32, f32, f32)], t: &Affine) -> f32 {
    points.iter().map(|&(x, y, r)| t.apply(x, y).1 + r).fold(f32::MIN, f32::max)
}

/// The rise that puts the body on the ground, given its lowest point
/// without any rise (0 = feet flat, as standing).
pub(crate) fn ground_rise(pose: &Pose, bottom: f32, standing_bottom: f32) -> f32 {
    pose.rise + pose.ground.clamp(0.0, 1.0) * (bottom - standing_bottom)
}

macro_rules! pose_lerp {
    ($a:expr, $b:expr, $t:expr; $($f:ident),*) => {
        Pose { $($f: $a.$f + ($b.$f - $a.$f) * $t),* }
    };
}

impl Pose {
    pub fn lerp(&self, o: &Pose, t: f32) -> Pose {
        pose_lerp!(self, o, t; rise, bob, lean, head, ls, le, rs, re, lh, lk, rh, rk, look, thumb, drop, fold, kneel, tilt, ground, sagittal)
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
    /// Lip-sync mouth shape (`Rest`: the expression's own mouth).
    pub mouth: crate::lipsync::Viseme,
}

/// A wearable part ready to draw: its ops (colours applied) and where it goes.
#[derive(Clone, Debug)]
pub struct Worn {
    pub anchor: Anchor,
    pub layer: Layer,
    pub hides_hair: bool,
    /// Worn on the face (eyewear): hidden from behind.
    pub face: bool,
    pub ops: Vec<Op>,
}

/// The worn parts for one anchor and layer, placed by `t`.
pub(crate) fn worn_at(wear: &[Worn], anchor: Anchor, layer: Layer, t: Affine) -> Option<Op> {
    let ops: Vec<Op> = wear.iter().filter(|w| w.anchor == anchor && w.layer == layer).flat_map(|w| w.ops.iter().cloned()).collect();
    (!ops.is_empty()).then(|| Op::group(t, 1.0, ops))
}

const INK: Color = Color::INK;
const MOUTH: Color = Color::rgb(0x5a, 0x1e, 0x2c);

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

/// Which way a character faces, as a yaw angle in degrees: 0 front, 90
/// facing screen-right (side), 180 back; negative = the mirrored left views.
pub fn view_parts(yaw: f32) -> (bool, f32) {
    let y = ((yaw + 180.0).rem_euclid(360.0)) - 180.0;
    (y < 0.0, y.abs())
}

/// Screen-x factor for a limb on `side` (-1 / +1) at a turn of `th` degrees
/// (0..180, facing +x): front = its own side, back = mirrored, side view =
/// forward for both. `sagittal` (walk cycles) makes angles mean forward/back.
pub(crate) fn limb_u(side: f32, th: f32, sagittal: f32) -> f32 {
    let c = th.to_radians().cos();
    let frontal = side * c + (1.0 - c.abs());
    frontal + (1.0 - frontal) * sagittal.clamp(0.0, 1.0)
}

/// Draw a character. Coordinates: feet at (0,0), about 330px tall. `yaw`
/// turns it (see `view_parts`); the left views mirror the right ones.
pub fn draw_character(look: &Look, pose: &Pose, face: &Face, wear: &[Worn], yaw: f32) -> Vec<Op> {
    let (mirror, th) = view_parts(yaw);
    let ops = if look.style == FigureStyle::Stick { crate::stick::draw_stick(look, pose, face, wear, th) } else { draw_cartoon(look, pose, face, wear, th) };
    if mirror {
        vec![Op::group(Affine::scale(-1.0, 1.0), 1.0, ops)]
    } else {
        ops
    }
}

fn draw_cartoon(look: &Look, pose: &Pose, face: &Face, wear: &[Worn], th: f32) -> Vec<Op> {
    let mut out = Vec::new();
    let (c, sn) = (th.to_radians().cos(), th.to_radians().sin());
    let back_view = th > 110.0;
    let width: f32 = match look.build {
        Build::Slim => 74.0,
        Build::Regular => 88.0,
        Build::Broad => 108.0,
    };
    let half = width / 2.0;
    // The body is narrower seen from the side.
    let thin = 0.62 + 0.38 * c.abs();
    let tw = width * thin;
    let th_half = tw / 2.0;
    let hip_y = -100.0 + pose.drop;
    let hip_x = (half - 22.0).max(14.0) * c;
    // Depth of each side: negative = away from the camera (drawn behind the body).
    let depth = |side: f32| -side * sn;
    let mut sides = [-1.0f32, 1.0];
    sides.sort_by(|a, b| depth(*a).total_cmp(&depth(*b)));
    // Circles (x, y, r) that can touch the ground, for the ground solver.
    let mut contact: Vec<(f32, f32, f32)> = Vec::new();

    // ---- legs (not affected by lean), far leg first ------------------------
    let mut legs = Vec::new();
    let thigh = THIGH * (1.0 - pose.fold.clamp(0.0, 0.9));
    let shin = SHIN * (1.0 - pose.kneel.clamp(0.0, 0.9));
    for side in sides {
        let (h, k) = if side < 0.0 { (pose.lh, pose.lk) } else { (pose.rh, pose.rk) };
        let u = limb_u(side, th, pose.sagittal);
        let start = (side * hip_x, hip_y);
        let foot = limb(&mut legs, start, h, k, thigh, shin, u, look.bottom, look.bottom, 25.0);
        // Toes point out to the side from the front and forward from the side.
        let toe = side * 7.0 * c + 9.0 * sn;
        legs.push(Op::fill(Path::ellipse(foot.0 + toe, foot.1 + 4.0, 18.0, 9.5), look.shoes));
        let d = dir(h, u);
        contact.extend([(start.0, start.1, 14.0), (start.0 + d.0 * thigh, start.1 + d.1 * thigh, 12.5), (foot.0 + toe, foot.1 + 4.0, 9.5)]);
    }

    // ---- upper body, rotated by lean around the hips -----------------------
    let mut upper = Vec::new();
    let top_y = -214.0;
    let head_c = (0.0f32, -272.0f32);
    let mut head_ops = Vec::new();
    let head_t = Affine::translate(head_c.0, head_c.1).then(&Affine::rotate_deg(pose.head));

    let body_sx = width / 88.0 * thin;
    let neck_t = Affine::translate(0.0, top_y).then(&Affine::scale(body_sx, 1.0));
    let torso_t = Affine::translate(0.0, top_y + 40.0).then(&Affine::scale(body_sx, 1.0));
    // Seen from behind, a cape is in front and a necklace behind.
    let (behind, in_front) = if back_view { (Layer::Front, Layer::Back) } else { (Layer::Back, Layer::Front) };
    for (a, t) in [(Anchor::Neck, neck_t), (Anchor::Torso, torso_t)] {
        upper.extend(worn_at(wear, a, behind, t));
    }
    upper.extend(worn_at(wear, Anchor::Head, Layer::Back, head_t.then(&head_part_t(th))));

    if look.hair_style == HairStyle::Long && !back_view {
        upper.push(Op::group(
            head_t,
            1.0,
            vec![Op::fill(Path::round_rect(-60.0 + sn * -10.0, -46.0, 120.0 - 30.0 * sn, 118.0, 44.0), look.hair.darken(0.08))],
        ));
    }

    // Arms: far ones behind the body, near (and front-view) ones in front.
    let sh_y = top_y + 16.0;
    let sh_x = (half - 9.0) * c;
    let mut hands = Vec::new();
    let mut arm = |out: &mut Vec<Op>, side: f32, hands: &mut Vec<(f32, f32, f32)>| {
        let (s, e) = if side < 0.0 { (pose.ls, pose.le) } else { (pose.rs, pose.re) };
        let u = limb_u(side, th, pose.sagittal);
        let hand = limb(out, (side * sh_x, sh_y), s, e, 42.0, 40.0, u, look.top, look.skin, 21.0);
        hands.push((hand.0, hand.1, 11.0));
        let anchor = if side > 0.0 { Anchor::HandR } else { Anchor::HandL };
        out.extend(worn_at(wear, anchor, Layer::Back, Affine::translate(hand.0, hand.1)));
        out.push(Op::fill(Path::circle(hand.0, hand.1, 11.0), look.skin));
        if side > 0.0 && pose.thumb > 0.5 {
            out.push(Op::fill(Path::round_rect(hand.0 - 4.0, hand.1 - 25.0, 8.5, 20.0, 4.2), look.skin));
        }
        out.extend(worn_at(wear, anchor, Layer::Front, Affine::translate(hand.0, hand.1)));
    };
    for side in sides.into_iter().filter(|s| depth(*s) < -0.3) {
        arm(&mut upper, side, &mut hands);
    }

    // Torso: trousers underneath, shirt on top.
    upper.push(Op::fill(Path::round_rect(-th_half, -150.0, tw, 62.0, 18.0), look.bottom));
    upper.push(Op::fill(Path::round_rect(-th_half, top_y, tw, 104.0, 30.0), look.top));
    upper.push(Op::fill(Path::rect(-th_half, -150.0, tw, 34.0), look.top));
    upper.push(Op::fill(Path::rect(-th_half, -118.0, tw, 6.0), look.bottom.darken(0.25)));
    // Front details slide toward the way the body faces and vanish from behind.
    let front_t = Affine::translate(sn * th_half * 0.45, 0.0).then(&Affine::scale(0.55 + 0.45 * c.max(0.0), 1.0));
    if c > 0.3 {
        upper.push(Op::group(front_t, 1.0, vec![Op::fill(Path::ellipse(0.0, top_y + 4.0, 20.0, 7.0), look.top.darken(0.18))]));
    }
    if look.accessory == Accessory::Tie && c > 0.15 {
        let mut p = Path::new();
        p.move_to(-7.0, top_y + 6.0)
            .line_to(7.0, top_y + 6.0)
            .line_to(10.0, top_y + 64.0)
            .line_to(0.0, top_y + 76.0)
            .line_to(-10.0, top_y + 64.0)
            .close();
        upper.push(Op::group(front_t, 1.0, vec![Op::fill(p, look.accent), Op::fill(Path::round_rect(-8.0, top_y + 2.0, 16.0, 12.0, 3.0), look.accent.darken(0.2))]));
    }

    upper.extend(worn_at(wear, Anchor::Torso, in_front, torso_t));
    for side in sides.into_iter().filter(|s| depth(*s) >= -0.3) {
        arm(&mut upper, side, &mut hands);
    }

    // Neck
    upper.push(Op::fill(Path::round_rect(-11.0, top_y - 16.0, 22.0, 22.0, 6.0), look.skin.darken(0.12)));
    upper.extend(worn_at(wear, Anchor::Neck, in_front, neck_t));

    // ---- head -------------------------------------------------------------
    if th < 1.0 {
        draw_head(&mut head_ops, look, pose, face, wear);
    } else {
        draw_head_turned(&mut head_ops, look, pose, face, wear, th);
    }
    let parts: Vec<Worn> = wear.iter().filter(|w| !(w.face && th > 100.0)).cloned().collect();
    head_ops.extend(worn_at(&parts, Anchor::Head, Layer::Front, head_part_t(th)));
    upper.push(Op::group(head_t, 1.0, head_ops));

    // The upper body is drawn around the standing hips (y -100); move it with
    // the dropped hips, then lean it.
    let lean_t = Affine::translate(0.0, hip_y + pose.bob)
        .then(&Affine::rotate_deg(pose.lean))
        .then(&Affine::translate(0.0, 100.0));
    let mut upper_pts = vec![(0.0, -120.0, th_half), (0.0, -190.0, th_half), (head_c.0, head_c.1, 50.0)];
    upper_pts.extend(hands);
    contact.extend(upper_pts.iter().map(|&(x, y, r)| {
        let (x, y) = lean_t.apply(x, y);
        (x, y, r)
    }));

    let tilt_t = Affine::translate(0.0, hip_y).then(&Affine::rotate_deg(pose.tilt)).then(&Affine::translate(0.0, -hip_y));
    let rise = if pose.ground > 0.0 { ground_rise(pose, lowest(&contact, &tilt_t), STANDING_BOTTOM) } else { pose.rise };
    out.push(Op::group(Affine::translate(0.0, -rise).then(&tilt_t), 1.0, {
        let mut v = legs;
        v.push(Op::group(lean_t, 1.0, upper));
        v
    }));
    out
}

/// Head-anchored parts follow the face a little as the head turns.
pub(crate) fn head_part_t(th: f32) -> Affine {
    let (c, sn) = (th.to_radians().cos(), th.to_radians().sin());
    if th < 1.0 {
        return Affine::IDENTITY;
    }
    Affine::translate(sn * 8.0, 0.0).then(&Affine::scale(0.72 + 0.28 * c.abs(), 1.0))
}

/// A turned head: features slide toward the way it faces, the far ones
/// disappear round the side, and from behind it is hair (or scalp) and ears.
fn draw_head_turned(out: &mut Vec<Op>, look: &Look, pose: &Pose, face: &Face, wear: &[Worn], th: f32) {
    let (c, sn) = (th.to_radians().cos(), th.to_radians().sin());
    let skin = look.skin;
    let hide_hair = wear.iter().any(|w| w.hides_hair);
    let bald = look.hair_style == HairStyle::Bald || hide_hair;
    // A point on the face plane `d` in front of the head's centre, turned.
    let proj = |fx: f32, d: f32| (fx * c + d * sn, -fx * sn + d * c);
    let back = th > 105.0;

    if look.hair_style == HairStyle::Bun && !hide_hair && !back {
        out.push(Op::fill(Path::circle(proj(0.0, -40.0).0, -56.0, 21.0), look.hair));
    }
    // Ears stick out at the sides when seen from (nearly) behind.
    if back {
        for side in [-1.0f32, 1.0] {
            out.push(Op::fill(Path::circle(side * 48.0 * c.abs().max(0.55), 4.0, 10.0), skin.darken(0.06)));
        }
    }
    out.push(Op::fill(Path::ellipse(0.0, 0.0, 50.0, 48.0), skin));
    // A nose that sticks out past the outline in profile.
    if !back && sn > 0.6 {
        out.push(Op::fill(Path::ellipse(proj(0.0, 47.0).0, 10.0, 7.0, 6.0), skin));
    }

    if back {
        if !bald {
            let hc = look.hair;
            out.push(Op::fill(Path::ellipse(0.0, -6.0, 52.0, 47.0), hc));
            match look.hair_style {
                HairStyle::Long => out.push(Op::fill(Path::round_rect(-54.0, -20.0, 108.0, 96.0, 40.0), hc)),
                HairStyle::Curly => {
                    for i in 0..9 {
                        let a = (165.0 + i as f32 * 26.0f32).to_radians();
                        out.push(Op::fill(Path::circle(a.cos() * 48.0, a.sin() * 44.0 - 6.0, 15.0), hc));
                    }
                }
                HairStyle::Spiky => {
                    let pts = [(-50.0, -20.0), (-46.0, -62.0), (-26.0, -54.0), (-16.0, -76.0), (0.0, -58.0), (16.0, -74.0), (26.0, -54.0), (46.0, -62.0), (50.0, -20.0)];
                    out.push(Op::fill(Path::polyline(&pts, true), hc));
                }
                HairStyle::Bun => out.push(Op::fill(Path::circle(0.0, -52.0, 21.0), hc.darken(0.05))),
                _ => {}
            }
        }
        // Back 3/4: a sliver of cheek and the near ear show at the front edge.
        if th < 170.0 {
            let (ex, _) = proj(-48.0, 0.0);
            out.push(Op::fill(Path::circle(ex, 4.0, 10.0), skin.darken(0.06)));
        }
        match look.accessory {
            Accessory::Cap => {
                let mut p = Path::new();
                p.move_to(-53.0, -14.0).cubic_to(-54.0, -66.0, 54.0, -66.0, 53.0, -14.0).close();
                out.push(Op::fill(p, look.accent));
            }
            Accessory::Bow => out.push(Op::fill(Path::circle(proj(30.0, -10.0).0, -46.0, 9.0), look.accent)),
            _ => {}
        }
        return;
    }

    // Hair at the back of the head, then the near ear on top of it.
    if !bald {
        let bx = -sn * 30.0;
        out.push(Op::fill(Path::ellipse(bx, -12.0, 22.0 + 10.0 * sn, 40.0), look.hair));
    }
    for side in [-1.0f32, 1.0] {
        let (x, z) = proj(side * 48.0, 0.0);
        if z > -8.0 {
            out.push(Op::fill(Path::circle(x, 4.0, 10.0), skin.darken(0.06)));
        }
    }

    // Eyes (and brows, cheeks, glasses) per side, squashed by the turn.
    let sq = 0.55 + 0.45 * c;
    for side in [-1.0f32, 1.0] {
        let (x, z) = proj(side * 18.0, 30.0);
        if z < -4.0 {
            continue;
        }
        let far = if z < 10.0 { 0.8 } else { 1.0 };
        let t = Affine::translate(x, 0.0).then(&Affine::scale(sq * far, 1.0)).then(&Affine::translate(-side * 18.0, 0.0));
        let mut ops = Vec::new();
        if matches!(face.expression, Expression::Happy | Expression::Wink) {
            ops.push(Op::fill(Path::ellipse(side * 29.0, 14.0, 9.0, 5.5), Color::rgba(0xf2, 0x7a, 0x86, 110)));
        }
        draw_eyes_side(&mut ops, look, pose, face, side);
        if look.accessory == Accessory::Glasses {
            ops.push(Op::stroke(Path::round_rect(side * 18.0 - 14.0, -12.0, 28.0, 24.0, 9.0), INK, 3.0));
        }
        out.push(Op::group(t, 1.0, ops));
    }
    // Facial hair follows the jaw, then the mouth and nose.
    let (bx, _) = proj(0.0, 12.0);
    let mut beard = Vec::new();
    draw_beard(&mut beard, look);
    out.push(Op::group(Affine::translate(bx, 0.0).then(&Affine::scale(0.5 + 0.5 * c, 1.0)), 1.0, beard));
    let (mx, _) = proj(0.0, 30.0);
    let mut ops = Vec::new();
    draw_mouth(&mut ops, look, face);
    draw_moustache(&mut ops, look);
    out.push(Op::group(Affine::translate(mx, 0.0).then(&Affine::scale(0.6 + 0.4 * c, 1.0)), 1.0, ops));
    let mut nose = Vec::new();
    draw_nose(&mut nose, look);
    out.push(Op::group(Affine::translate(proj(0.0, 42.0).0, 0.0).then(&Affine::scale(0.8, 1.0)), 1.0, nose));

    // The hairline cap slides back as the head turns. Long hair keeps only
    // the strand on the far (back) side; the front one would cover the face.
    if !bald {
        let mut hair = Vec::new();
        if look.hair_style == HairStyle::Long {
            let cap = Look { hair_style: HairStyle::Short, ..look.clone() };
            draw_hair(&mut hair, &cap);
        } else {
            draw_hair(&mut hair, look);
        }
        out.push(Op::group(Affine::translate(-sn * 6.0, 0.0).then(&Affine::scale(0.62 + 0.38 * c, 1.0)), 1.0, hair));
        if look.hair_style == HairStyle::Long {
            out.push(Op::fill(Path::round_rect(-sn * 44.0 - 9.0 - 40.0 * c, -20.0, 18.0 + 30.0 * c, 64.0, 9.0), look.hair));
        }
    }
    match look.accessory {
        Accessory::Cap => {
            let mut p = Path::new();
            p.move_to(-53.0, -14.0).cubic_to(-54.0, -66.0, 54.0, -66.0, 53.0, -14.0).close();
            out.push(Op::group(Affine::scale(0.8 + 0.2 * c, 1.0), 1.0, vec![Op::fill(p, look.accent), Op::fill(Path::round_rect(-6.0, -20.0, 76.0, 12.0, 6.0), look.accent.darken(0.25))]));
        }
        Accessory::Bow => {
            let (bx, by) = (proj(30.0, -10.0).0, -46.0);
            out.push(Op::fill(Path::polyline(&[(bx, by), (bx - 16.0, by - 12.0), (bx - 16.0, by + 12.0)], true), look.accent));
            out.push(Op::fill(Path::polyline(&[(bx, by), (bx + 16.0, by - 12.0), (bx + 16.0, by + 12.0)], true), look.accent));
            out.push(Op::fill(Path::circle(bx, by, 6.0), look.accent.darken(0.2)));
        }
        _ => {}
    }
}

/// Cartoon leg lengths and the lowest point of a standing figure (shoe soles).
pub const THIGH: f32 = 46.0;
pub const SHIN: f32 = 44.0;
const STANDING_BOTTOM: f32 = 3.5;

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
    draw_mouth(out, look, face);
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
    for side in [-1.0f32, 1.0] {
        draw_eyes_side(out, look, pose, face, side);
    }
}

/// One eye and its brow.
fn draw_eyes_side(out: &mut Vec<Op>, look: &Look, pose: &Pose, face: &Face, side: f32) {
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
    {
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
            BrowStyle::None => return,
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

fn draw_mouth(out: &mut Vec<Op>, look: &Look, face: &Face) {
    use Expression::*;
    let (mx, my) = (0.0, 26.0);
    if face.mouth != crate::lipsync::Viseme::Rest {
        crate::mouths::draw(out, look, face.mouth, mx, my);
        return;
    }
    let lips = look.mouth == crate::model::MouthSet::Lips;
    if lips {
        crate::mouths::rest_lips(out, look, face.expression, mx, my);
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

// ---------------------------------------------------------------------------
// Walk and run cycles
// ---------------------------------------------------------------------------

/// Distance the body travels in one full walk cycle (two steps), in
/// character units at scale 1. Driving the cycle by distance (see
/// `locomotion`) keeps the standing foot planted on the ground.
pub const WALK_CYCLE: f32 = 68.0;
pub const RUN_CYCLE: f32 = 112.0;

/// Gait parameters.
struct Gait {
    /// One step, body travel in px.
    step: f32,
    /// Fraction of the cycle each foot is on the ground (> 0.5 walks, < 0.5 runs).
    stance: f32,
    lift: f32,
    /// Hip drop at contact and down (on top of what the stride needs), then at passing and up (px).
    drops: [f32; 4],
    arm: f32,
    elbow: f32,
    lean: f32,
    /// Peak airborne lift between steps (running).
    flight: f32,
}

const WALK: Gait = Gait { step: WALK_CYCLE / 2.0, stance: 0.6, lift: 9.0, drops: [0.6, 3.2, 1.2, 0.0], arm: 16.0, elbow: 14.0, lean: 1.5, flight: 0.0 };
const RUN: Gait = Gait { step: RUN_CYCLE / 2.0, stance: 0.38, lift: 24.0, drops: [4.0, 9.0, 5.0, 3.0], arm: 34.0, elbow: 76.0, lean: 7.0, flight: 8.0 };

/// Two-bone IK in a leg's own space: (u outward, v down) from hip to foot.
/// `bend` = +1 puts the knee toward +u. Returns (hip angle, knee angle).
fn leg_ik(u: f32, v: f32, l1: f32, l2: f32, bend: f32) -> (f32, f32) {
    let d = (u * u + v * v).sqrt().clamp(1.0, l1 + l2 - 0.01);
    let theta = u.atan2(v);
    let alpha = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0).acos();
    let a1 = theta + bend * alpha;
    // Knee, then the shin's absolute angle to the foot.
    let (ku, kv) = (a1.sin() * l1, a1.cos() * l1);
    let s = (u - ku).atan2(v - kv);
    (a1.to_degrees(), (s - a1).to_degrees())
}

/// Smooth curve through the four step keys (contact, down, passing, up).
fn step_curve(keys: [f32; 4], phi: f32) -> f32 {
    let x = phi.rem_euclid(1.0) * 4.0;
    let i = x.floor() as usize % 4;
    let t = x - x.floor();
    let t = t * t * (3.0 - 2.0 * t);
    keys[i] + (keys[(i + 1) % 4] - keys[i]) * t
}

/// A walk (or run) pose at `phase` (0..1 = one cycle of two steps). `fwd` is
/// the walking direction in the character's own x (+1 or -1). Classic 2D
/// breakdown: contact, down, passing, up for each step; the standing foot
/// slides back at exactly the body's speed, so feet stay planted when the
/// phase follows distance travelled.
pub fn locomotion(phase: f32, run: bool, fwd: f32) -> Pose {
    locomotion_stride(phase, run, fwd, 1.0)
}

/// The walk seen from the side (or 3/4): the same steps, but limb angles
/// mean forward and back (`Pose::sagittal`), facing +x.
pub fn locomotion_side(phase: f32, run: bool, stride: f32) -> Pose {
    gait(phase, run, 1.0, stride, true)
}

/// `locomotion` with a longer stride for faster walks (`stride` 1..1.6): the
/// steps get longer and the hips sink a little more to reach them.
pub fn locomotion_stride(phase: f32, run: bool, fwd: f32, stride: f32) -> Pose {
    gait(phase, run, fwd, stride, false)
}

fn gait(phase: f32, run: bool, fwd: f32, stride: f32, side_view: bool) -> Pose {
    let g = if run { &RUN } else { &WALK };
    let k = stride.clamp(1.0, 1.6);
    let fwd = if fwd < 0.0 { -1.0 } else { 1.0 };
    let reach = THIGH + SHIN;
    // How far the foot moves back (in the hip's frame) while it is down.
    let span = g.step * k * 2.0 * g.stance;
    let step_phase = (phase * 2.0).rem_euclid(1.0);
    // The hips sink just enough to reach the longest step, then bounce:
    // lowest just after contact (down), highest after passing (up).
    let need = reach - (reach * reach - (span / 2.0).powi(2)).max(0.0).sqrt();
    let drops = [need + g.drops[0], need + g.drops[1], g.drops[2], g.drops[3]];
    let drop = step_curve(drops, step_phase);
    let mut p = Pose { drop, lean: fwd * g.lean, sagittal: if side_view { 1.0 } else { 0.0 }, ..Pose::default() };
    // Front view: each limb swings out on its own side; side view: forward is +x.
    let along = |side: f32| if side_view { 1.0 } else { side * fwd };

    // Legs: left foot leads at phase 0, right foot half a cycle later.
    let mut foot_fwd = [0.0f32; 2];
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let lp = (phase + i as f32 * 0.5).rem_euclid(1.0);
        let (fx, lift) = if lp < g.stance {
            (span / 2.0 - span * lp / g.stance, 0.0)
        } else {
            let q = (lp - g.stance) / (1.0 - g.stance);
            let s = (1.0 - (std::f32::consts::PI * q).cos()) / 2.0;
            (-span / 2.0 + span * s, g.lift * (std::f32::consts::PI * q).sin())
        };
        foot_fwd[i] = fx / (span / 2.0);
        let u = along(side) * fx;
        let v = reach - drop - lift;
        let (h, k) = leg_ik(u, v, THIGH, SHIN, along(side));
        if side < 0.0 {
            p.lh = h;
            p.lk = k;
        } else {
            p.rh = h;
            p.rk = k;
        }
    }

    // Arms swing against the legs: the left arm comes forward with the right foot.
    for (side, opposite) in [(-1.0f32, foot_fwd[1]), (1.0, foot_fwd[0])] {
        let swing = g.arm * opposite;
        let bend = g.elbow + if run { 0.0 } else { 10.0 * opposite.max(0.0) };
        let out = if run || side_view { 0.0 } else { 4.0 };
        let (s, e) = (along(side) * swing + out, along(side) * bend);
        if side < 0.0 {
            p.ls = s;
            p.le = e;
        } else {
            p.rs = s;
            p.re = e;
        }
    }

    // Running: both feet leave the ground at the end of each step.
    if g.flight > 0.0 && g.stance < 0.5 {
        let start = 2.0 * g.stance;
        if step_phase >= start {
            let q = (step_phase - start) / (1.0 - start);
            p.rise = g.flight * (std::f32::consts::PI * q).sin();
        }
    }
    // The head bobs a touch against the body and looks where it is going.
    p.head = -fwd * 1.0 + step_curve([0.0, 1.2, 0.0, -0.8], step_phase);
    p.look = fwd * 0.6;
    p
}

#[cfg(test)]
mod gait_tests {
    use super::*;

    /// Where a foot is in the character's own space for a pose.
    fn foot(p: &Pose, side: f32) -> (f32, f32) {
        let (h, k) = if side < 0.0 { (p.lh, p.lk) } else { (p.rh, p.rk) };
        let d1 = dir(h, side);
        let d2 = dir(h + k, side);
        let hip = (0.0, -100.0 + p.drop);
        (hip.0 + d1.0 * THIGH + d2.0 * SHIN, hip.1 + d1.1 * THIGH + d2.1 * SHIN - p.rise)
    }

    #[test]
    fn walking_feet_stay_planted() {
        // While a foot is down, its world position (body x + foot x) must not move.
        let frames = 60;
        for side in [-1.0f32, 1.0] {
            let mut planted: Vec<(f32, f32)> = vec![];
            for f in 0..frames {
                let phase = f as f32 / frames as f32;
                let body_x = phase * WALK_CYCLE;
                let p = locomotion(phase, false, 1.0);
                let (fx, fy) = foot(&p, side);
                if fy > -10.5 {
                    planted.push((body_x + fx, fy));
                }
            }
            assert!(planted.len() > frames / 3, "foot {side} is down for a good part of the cycle");
            // Group consecutive contacts; within one contact the x must hold still.
            let xs: Vec<f32> = planted.iter().map(|p| p.0).collect();
            let spread = |v: &[f32]| v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min);
            let mut run = vec![xs[0]];
            for w in xs.windows(2) {
                if (w[1] - w[0]).abs() > 20.0 {
                    assert!(spread(&run) < 1.5, "foot {side} slides {} px", spread(&run));
                    run.clear();
                }
                run.push(w[1]);
            }
            assert!(spread(&run) < 1.5, "foot {side} slides {} px", spread(&run));
        }
    }

    #[test]
    fn side_view_feet_stay_planted() {
        // Side view: limb angles are forward/back, so a foot's x is +sin.
        let frames = 60;
        let mut xs = vec![];
        for f in 0..frames {
            let phase = f as f32 / frames as f32;
            let p = locomotion_side(phase, false, 1.0);
            let (a1, a2) = (p.lh.to_radians(), (p.lh + p.lk).to_radians());
            let x = a1.sin() * THIGH + a2.sin() * SHIN;
            let y = -100.0 + p.drop + a1.cos() * THIGH + a2.cos() * SHIN;
            if y > -10.5 {
                xs.push(phase * WALK_CYCLE + x);
            }
        }
        let mut run = vec![xs[0]];
        for w in xs.windows(2) {
            if (w[1] - w[0]).abs() > 20.0 {
                let spread = run.iter().cloned().fold(f32::MIN, f32::max) - run.iter().cloned().fold(f32::MAX, f32::min);
                assert!(spread < 1.5, "slides {spread}");
                run.clear();
            }
            run.push(w[1]);
        }
    }

    #[test]
    fn arms_swing_against_legs_and_body_bounces() {
        let a = locomotion(0.0, false, 1.0);
        let b = locomotion(0.5, false, 1.0);
        assert!((a.ls - b.ls).abs() > 20.0 && (a.rs - b.rs).abs() > 20.0, "arms swing");
        let drops: Vec<f32> = (0..8).map(|i| locomotion(i as f32 / 16.0, false, 1.0).drop).collect();
        let (lo, hi) = (drops.iter().cloned().fold(f32::MAX, f32::min), drops.iter().cloned().fold(f32::MIN, f32::max));
        assert!(hi - lo > 4.0, "the body rises and falls");
        // Mirrored direction mirrors the legs.
        let m = locomotion(0.2, false, -1.0);
        let n = locomotion(0.2, false, 1.0);
        assert!((m.lh + n.lh).abs() < 0.01 && (m.rk + n.rk).abs() < 0.01);
    }
}
