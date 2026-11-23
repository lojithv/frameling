//! Stick figures: the same skeleton, poses and faces as the cartoon rig, drawn
//! as round-capped ink lines with a circle head. Feet at (0,0), y grows down,
//! about 300px tall, so they stand next to cartoon characters at the same scale.
//!
//! Colours from the `Look`: `top` = line ink, `skin` = head fill, `hair` = hair,
//! `accent` = dress / bow / cap / tie.

use crate::display::{Op, Path};
use crate::geom::{Affine, Color};
use crate::model::{Accessory, Build, Expression, HairStyle, Look};
use crate::pack::{Anchor, Layer};
use crate::rig::{wavy_mouth, worn_at, Face, Pose, Worn};
use crate::model::FacialHair;

const HIP_Y: f32 = -100.0;
const NECK_Y: f32 = -204.0;
const SHOULDER_Y: f32 = -192.0;
const HEAD_R: f32 = 40.0;
const HEAD_Y: f32 = NECK_Y - HEAD_R + 2.0;

fn dir(angle_deg: f32, side: f32) -> (f32, f32) {
    let r = angle_deg.to_radians();
    (side * r.sin(), r.cos())
}

/// Two-segment limb from `start`. Returns (joint, end).
fn bone(start: (f32, f32), a1: f32, a2: f32, l1: f32, l2: f32, side: f32) -> ((f32, f32), (f32, f32)) {
    let d1 = dir(a1, side);
    let mid = (start.0 + d1.0 * l1, start.1 + d1.1 * l1);
    let d2 = dir(a1 + a2, side);
    (mid, (mid.0 + d2.0 * l2, mid.1 + d2.1 * l2))
}

/// Stick parts are authored at cartoon size: the head is 40 px instead of 50,
/// and the body is a line, so neck and torso parts shrink.
const HEAD_K: f32 = HEAD_R / 50.0;
const BODY_K: f32 = 0.62;

pub fn draw_stick(look: &Look, pose: &Pose, face: &Face, wear: &[Worn]) -> Vec<Op> {
    let ink = look.top;
    let w: f32 = match look.build {
        Build::Slim => 7.0,
        Build::Regular => 9.0,
        Build::Broad => 12.0,
    };

    // ---- legs (not affected by lean) -------------------------------------
    let hip_y = HIP_Y + pose.drop;
    let mut contact: Vec<(f32, f32, f32)> = Vec::new();
    let mut legs = Vec::new();
    let thigh = 47.0 * (1.0 - pose.fold.clamp(0.0, 0.9));
    let shin = 46.0 * (1.0 - pose.kneel.clamp(0.0, 0.9));
    for (side, h, k) in [(-1.0f32, pose.lh, pose.lk), (1.0, pose.rh, pose.rk)] {
        let start = (side * 3.0, hip_y);
        // A slight natural stance so the two legs never draw as one line.
        let (knee, foot) = bone(start, h + 6.0, k - 4.0, thigh, shin, side);
        contact.extend([(start.0, start.1, w), (knee.0, knee.1, w * 0.6), (foot.0, foot.1, w * 0.6)]);
        legs.push(Op::stroke(Path::polyline(&[start, knee, foot], false), ink, w));
        // A short foot pointing outwards, following the shin a little.
        let toe = (foot.0 + side * 13.0, foot.1 + 2.0);
        legs.push(Op::stroke(Path::polyline(&[foot, toe], false), look.shoes, w));
    }

    // ---- upper body, rotated by lean around the hips -----------------------
    let mut upper = Vec::new();
    let head_c = (0.0f32, HEAD_Y);
    let head_t = Affine::translate(head_c.0, head_c.1).then(&Affine::rotate_deg(pose.head));
    let part_head_t = head_t.then(&Affine::scale(HEAD_K, HEAD_K));
    let neck_t = Affine::translate(0.0, NECK_Y).then(&Affine::scale(BODY_K, BODY_K));
    let torso_t = Affine::translate(0.0, NECK_Y + 34.0).then(&Affine::scale(BODY_K, BODY_K));
    for (a, t) in [(Anchor::Neck, neck_t), (Anchor::Torso, torso_t), (Anchor::Head, part_head_t)] {
        upper.extend(worn_at(wear, a, Layer::Back, t));
    }

    // Long hair hangs behind the head and shoulders.
    if look.hair_style == HairStyle::Long {
        let mut back = Vec::new();
        for side in [-1.0f32, 1.0] {
            let mut p = Path::new();
            p.move_to(side * 30.0, -26.0).cubic_to(side * 58.0, -8.0, side * 52.0, 34.0, side * 44.0, 58.0);
            back.push(Op::stroke(p, look.hair, 7.0));
            let mut p = Path::new();
            p.move_to(side * 20.0, -34.0).cubic_to(side * 50.0, -14.0, side * 40.0, 30.0, side * 32.0, 50.0);
            back.push(Op::stroke(p, look.hair, 6.0));
        }
        upper.push(Op::group(head_t, 1.0, back));
    }

    // Spine
    upper.push(Op::stroke(Path::polyline(&[(0.0, HIP_Y), (0.0, NECK_Y)], false), ink, w));

    // Dress: an A-line triangle from the chest to just above the knees.
    if look.dress {
        let mut p = Path::new();
        p.move_to(-9.0, SHOULDER_Y + 6.0)
            .line_to(9.0, SHOULDER_Y + 6.0)
            .line_to(46.0, HIP_Y + 22.0)
            .quad_to(0.0, HIP_Y + 30.0, -46.0, HIP_Y + 22.0)
            .close();
        upper.push(Op::fill(p.clone(), look.accent));
        upper.push(Op::stroke(p, ink, (w * 0.55).max(4.0)));
    }

    if look.accessory == Accessory::Tie {
        let mut p = Path::new();
        p.move_to(-6.0, NECK_Y + 6.0).line_to(6.0, NECK_Y + 6.0).line_to(8.0, NECK_Y + 46.0).line_to(0.0, NECK_Y + 56.0).line_to(-8.0, NECK_Y + 46.0).close();
        upper.push(Op::fill(p, look.accent));
    }

    upper.extend(worn_at(wear, Anchor::Torso, Layer::Front, torso_t));
    upper.extend(worn_at(wear, Anchor::Neck, Layer::Front, neck_t));

    // Arms
    let mut hands = Vec::new();
    for (side, s, e) in [(-1.0f32, pose.ls, pose.le), (1.0, pose.rs, pose.re)] {
        let start = (side * 2.0, SHOULDER_Y);
        let (elbow, hand) = bone(start, s, e, 44.0, 42.0, side);
        hands.push((hand.0, hand.1, w));
        let anchor = if side > 0.0 { Anchor::HandR } else { Anchor::HandL };
        let hand_t = Affine::translate(hand.0, hand.1).then(&Affine::scale(0.85, 0.85));
        upper.extend(worn_at(wear, anchor, Layer::Back, hand_t));
        upper.push(Op::stroke(Path::polyline(&[start, elbow, hand], false), ink, w));
        upper.push(Op::fill(Path::circle(hand.0, hand.1, w * 0.75), ink));
        upper.extend(worn_at(wear, anchor, Layer::Front, hand_t));
        if side > 0.0 && pose.thumb > 0.5 {
            // Fist with the thumb pointing up, whatever the arm angle.
            upper.push(Op::fill(Path::circle(hand.0, hand.1, w * 1.05), ink));
            upper.push(Op::stroke(Path::polyline(&[(hand.0, hand.1 - 2.0), (hand.0, hand.1 - 20.0)], false), ink, w * 0.7));
        }
    }

    // ---- head -------------------------------------------------------------
    let mut head = Vec::new();
    draw_head(&mut head, look, pose, face, ink, w, wear.iter().any(|x| x.hides_hair));
    head.extend(worn_at(wear, Anchor::Head, Layer::Front, Affine::scale(HEAD_K, HEAD_K)));
    upper.push(Op::group(head_t, 1.0, head));

    let lean_t = Affine::translate(0.0, hip_y + pose.bob).then(&Affine::rotate_deg(pose.lean)).then(&Affine::translate(0.0, -HIP_Y));
    let mut upper_pts = vec![(0.0, -150.0, w), (0.0, HEAD_Y, HEAD_R)];
    upper_pts.extend(hands);
    contact.extend(upper_pts.iter().map(|&(x, y, r)| {
        let (x, y) = lean_t.apply(x, y);
        (x, y, r)
    }));
    let tilt_t = Affine::translate(0.0, hip_y).then(&Affine::rotate_deg(pose.tilt)).then(&Affine::translate(0.0, -hip_y));
    // A standing stick figure's lowest point: the feet, about 7 px above 0 plus the line width.
    let standing = -7.0 + w * 0.6;
    let rise = if pose.ground > 0.0 { crate::rig::ground_rise(pose, crate::rig::lowest(&contact, &tilt_t), standing) } else { pose.rise };
    let mut all = legs;
    all.push(Op::group(lean_t, 1.0, upper));
    vec![Op::group(Affine::translate(0.0, -rise).then(&tilt_t), 1.0, all)]
}

fn draw_head(out: &mut Vec<Op>, look: &Look, pose: &Pose, face: &Face, ink: Color, w: f32, hide_hair: bool) {
    let r = HEAD_R;
    if look.hair_style == HairStyle::Bun {
        out.push(Op::fill(Path::circle(0.0, -r - 10.0, 15.0), look.hair));
    }
    out.push(Op::fill(Path::circle(0.0, 0.0, r), look.skin));
    out.push(Op::stroke(Path::circle(0.0, 0.0, r), ink, (w * 0.8).max(5.0)));

    draw_face(out, pose, face, ink);
    draw_facial_hair(out, look);
    if !hide_hair {
        draw_hair(out, look);
    }

    match look.accessory {
        Accessory::Glasses => {
            for side in [-1.0f32, 1.0] {
                out.push(Op::stroke(Path::circle(side * 14.0, -4.0, 10.0), ink, 3.0));
            }
            out.push(Op::stroke(Path::polyline(&[(-4.0, -5.0), (4.0, -5.0)], false), ink, 3.0));
        }
        Accessory::Cap => {
            let mut p = Path::new();
            p.move_to(-r - 1.0, -12.0).cubic_to(-r, -r * 1.35, r, -r * 1.35, r + 1.0, -12.0).close();
            out.push(Op::fill(p, look.accent));
            out.push(Op::fill(Path::round_rect(-2.0, -18.0, r + 26.0, 10.0, 5.0), look.accent.darken(0.25)));
        }
        Accessory::Bow => {
            let (bx, by) = (24.0, -r + 2.0);
            out.push(Op::fill(Path::polyline(&[(bx, by), (bx - 16.0, by - 11.0), (bx - 16.0, by + 11.0)], true), look.accent));
            out.push(Op::fill(Path::polyline(&[(bx, by), (bx + 16.0, by - 11.0), (bx + 16.0, by + 11.0)], true), look.accent));
            out.push(Op::fill(Path::circle(bx, by, 5.0), look.accent.darken(0.2)));
        }
        _ => {}
    }
}

fn draw_face(out: &mut Vec<Op>, pose: &Pose, face: &Face, ink: Color) {
    use Expression::*;
    let (ex, ey) = (14.0, -6.0);
    let mut px = pose.look * 3.0;
    let mut py = 0.0;
    match face.expression {
        Thinking => {
            px = 3.0;
            py = -3.0;
        }
        Sad | Worried => py = 2.0,
        Confused => px = -2.0,
        _ => {}
    }
    for side in [-1.0f32, 1.0] {
        let cx = side * ex;
        let arc = |up: bool| {
            let mut p = Path::new();
            let dy = if up { -6.0 } else { 3.0 };
            p.move_to(cx - 7.0, ey + 1.0).quad_to(cx, ey + dy, cx + 7.0, ey + 1.0);
            Op::stroke(p, ink, 3.5)
        };
        let wink_closed = face.expression == Wink && side < 0.0;
        if face.expression == Happy || wink_closed {
            out.push(arc(true));
        } else if face.blink {
            out.push(arc(false));
        } else if face.expression == Surprised {
            out.push(Op::stroke(Path::circle(cx, ey, 6.5), ink, 3.0));
            out.push(Op::fill(Path::circle(cx + px, ey + py, 2.5), ink));
        } else {
            out.push(Op::fill(Path::circle(cx + px, ey + py, 4.5), ink));
        }
        // Brows only when they say something.
        let brow = match face.expression {
            Angry => Some((-13.0, -20.0)),
            Sad | Worried => Some((-22.0, -16.0)),
            Surprised => Some((-24.0, -23.0)),
            Thinking if side > 0.0 => Some((-22.0, -21.0)),
            Confused if side > 0.0 => Some((-25.0, -22.0)),
            Confused => Some((-15.0, -15.0)),
            _ => None,
        };
        if let Some((inner, outer)) = brow {
            out.push(Op::stroke(Path::polyline(&[(side * 6.0, inner), (side * 21.0, outer)], false), ink, 3.5));
        }
    }

    let (mx, my) = (0.0, 16.0);
    if face.mouth > 0 {
        let l = face.mouth as f32;
        out.push(Op::fill(Path::ellipse(mx, my, 6.0 + 3.0 * l, 2.5 + 3.2 * l), ink));
        return;
    }
    let mut p = Path::new();
    match face.expression {
        Happy | Wink => {
            p.move_to(mx - 14.0, my - 3.0).quad_to(mx, my + 14.0, mx + 14.0, my - 3.0);
            out.push(Op::stroke(p, ink, 3.5));
        }
        Sad => {
            p.move_to(mx - 11.0, my + 6.0).quad_to(mx, my - 4.0, mx + 11.0, my + 6.0);
            out.push(Op::stroke(p, ink, 3.5));
        }
        Angry => out.push(Op::stroke(Path::polyline(&[(mx - 11.0, my + 3.0), (mx + 11.0, my + 3.0)], false), ink, 4.0)),
        Surprised => out.push(Op::stroke(Path::ellipse(mx, my + 3.0, 5.5, 7.0), ink, 3.0)),
        Thinking => out.push(Op::stroke(Path::polyline(&[(mx - 5.0, my + 2.0), (mx + 10.0, my - 1.0)], false), ink, 3.5)),
        Worried | Confused => out.push(Op::stroke(wavy_mouth(mx, my + 2.0, 11.0), ink, 3.0)),
        Neutral => {
            p.move_to(mx - 9.0, my).quad_to(mx, my + 5.0, mx + 9.0, my);
            out.push(Op::stroke(p, ink, 3.5));
        }
    }
}

fn draw_hair(out: &mut Vec<Op>, look: &Look) {
    let c = look.hair;
    let r = HEAD_R;
    match look.hair_style {
        HairStyle::Bald => {}
        HairStyle::Short | HairStyle::Bun | HairStyle::Long => {
            // A swept fringe over the top of the head.
            let mut p = Path::new();
            p.move_to(-r + 2.0, -10.0).cubic_to(-r + 2.0, -r - 6.0, r - 2.0, -r - 6.0, r - 2.0, -10.0).cubic_to(r - 14.0, -r + 18.0, -4.0, -r + 16.0, -r + 2.0, -10.0).close();
            out.push(Op::fill(p, c));
        }
        HairStyle::Spiky => {
            let pts = [(-30.0, -26.0), (-30.0, -50.0), (-15.0, -36.0), (-6.0, -58.0), (4.0, -38.0), (16.0, -56.0), (20.0, -34.0), (34.0, -46.0), (30.0, -24.0)];
            out.push(Op::stroke(Path::polyline(&pts, false), c, 5.0));
        }
        HairStyle::Curly => {
            for i in 0..7 {
                let a = (205.0 + i as f32 * 21.5f32).to_radians();
                out.push(Op::stroke(Path::circle(a.cos() * (r + 2.0), a.sin() * (r + 2.0), 9.0), c, 4.0));
            }
        }
    }
}

/// Line-drawn facial hair in the hair colour.
fn draw_facial_hair(out: &mut Vec<Op>, look: &Look) {
    let c = look.hair;
    let tache = |out: &mut Vec<Op>| {
        let mut p = Path::new();
        p.move_to(-14.0, 14.0).quad_to(0.0, 6.0, 14.0, 14.0);
        out.push(Op::stroke(p, c, 4.5));
    };
    match look.facial_hair {
        FacialHair::None => {}
        FacialHair::Stubble => {
            for i in 0..9 {
                let a = (25.0 + i as f32 * 16.0f32).to_radians();
                out.push(Op::fill(Path::circle(a.cos() * 30.0, a.sin() * 30.0 + 4.0, 1.6), c));
            }
        }
        FacialHair::Moustache => tache(out),
        FacialHair::Goatee => {
            tache(out);
            out.push(Op::stroke(Path::polyline(&[(0.0, 30.0), (0.0, 40.0)], false), c, 6.0));
        }
        FacialHair::Beard | FacialHair::FullBeard => {
            tache(out);
            let drop = if look.facial_hair == FacialHair::FullBeard { 58.0 } else { 48.0 };
            let mut p = Path::new();
            p.move_to(-38.0, 8.0).cubic_to(-34.0, drop, 34.0, drop, 38.0, 8.0);
            out.push(Op::stroke(p, c, 5.0));
        }
    }
}
