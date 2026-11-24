//! Lip-sync mouth shapes, drawn for each mouth set.
//!
//! Every set draws all nine shapes of [`Viseme`], centred on the mouth point
//! of the head, so any set fits any character. Sets: classic (a dark opening
//! with teeth and tongue) and lips (the same openings framed by full coloured
//! lips). Stick figures have their own line-drawn shapes.

use crate::display::{Op, Path};
use crate::geom::Color;
use crate::lipsync::Viseme;
use crate::model::{Expression, Look, MouthSet};

const MOUTH: Color = Color::rgb(0x5a, 0x1e, 0x2c);
const TONGUE: Color = Color::rgb(0xe8, 0x6f, 0x7e);
const TEETH: Color = Color::rgb(0xfb, 0xfa, 0xf6);

/// An open mouth: the dark opening, where its teeth show and its tongue.
struct Opening {
    rx: f32,
    ry: f32,
    /// Centre below the mouth point.
    dy: f32,
    /// Teeth together (a narrow box of teeth with a line between them).
    clenched: bool,
    /// Show the top teeth.
    teeth: bool,
    /// Tongue (offset below the centre, half width, half height).
    tongue: Option<(f32, f32, f32)>,
}

fn opening(v: Viseme) -> Option<Opening> {
    let o = |rx, ry, dy, teeth, tongue| Some(Opening { rx, ry, dy, clenched: false, teeth, tongue });
    match v {
        Viseme::Ee => Some(Opening { rx: 13.0, ry: 4.6, dy: 0.0, clenched: true, teeth: true, tongue: None }),
        Viseme::Eh => o(12.0, 8.0, 1.0, true, Some((4.5, 7.0, 3.0))),
        Viseme::Ai => o(14.0, 13.0, 3.0, true, Some((7.5, 8.5, 4.5))),
        Viseme::O => o(9.0, 11.0, 2.0, false, Some((6.5, 5.0, 3.0))),
        Viseme::U => o(5.0, 5.5, 1.0, false, None),
        Viseme::L => o(12.0, 10.0, 1.0, true, Some((-2.5, 6.0, 4.0))),
        _ => None,
    }
}

/// Lip colour for the lips set: a rosy tone that suits the skin.
fn lip_color(look: &Look) -> Color {
    look.skin.mix(Color::rgb(0xc2, 0x3c, 0x58), 0.55)
}

fn draw_opening(out: &mut Vec<Op>, o: &Opening, mx: f32, my: f32) {
    let cy = my + o.dy;
    if o.clenched {
        let (w, h) = (o.rx * 2.0, o.ry * 2.0);
        out.push(Op::fill(Path::round_rect(mx - o.rx, cy - o.ry, w, h, o.ry), MOUTH));
        out.push(Op::fill(Path::round_rect(mx - o.rx + 2.5, cy - o.ry + 1.2, w - 5.0, h - 2.4, o.ry - 1.2), TEETH));
        out.push(Op::stroke(Path::polyline(&[(mx - o.rx + 3.0, cy), (mx + o.rx - 3.0, cy)], false), MOUTH.with_alpha(0.55), 1.2));
        return;
    }
    out.push(Op::fill(Path::ellipse(mx, cy, o.rx, o.ry), MOUTH));
    if let Some((ty, tw, th)) = o.tongue {
        out.push(Op::fill(Path::ellipse(mx, cy + ty, tw.min(o.rx * 0.75), th.min(o.ry * 0.5)), TONGUE));
    }
    if o.teeth {
        // A band of top teeth tucked under the upper edge.
        out.push(Op::fill(Path::ellipse(mx, cy - o.ry * 0.62, o.rx * 0.66, o.ry * 0.3), TEETH));
    }
}

/// Upper lip with a little bow, from x = -w to w, its lower edge at `y`.
fn upper_lip(mx: f32, y: f32, w: f32, h: f32) -> Path {
    let mut p = Path::new();
    p.move_to(mx - w, y)
        .quad_to(mx - w * 0.5, y - h * 1.1, mx, y - h * 0.55)
        .quad_to(mx + w * 0.5, y - h * 1.1, mx + w, y)
        .quad_to(mx, y + h * 0.25, mx - w, y)
        .close();
    p
}

/// Lower lip from x = -w to w, its upper edge at `y`.
fn lower_lip(mx: f32, y: f32, w: f32, h: f32) -> Path {
    let mut p = Path::new();
    p.move_to(mx - w, y).quad_to(mx, y + h * 1.6, mx + w, y).quad_to(mx, y + h * 0.3, mx - w, y).close();
    p
}

/// Draw a lip-sync mouth shape (any shape but `Rest`) for a cartoon figure.
pub fn draw(out: &mut Vec<Op>, look: &Look, v: Viseme, mx: f32, my: f32) {
    let lips = look.mouth == MouthSet::Lips;
    let lip = lip_color(look);
    match v {
        Viseme::Rest => {}
        Viseme::Mbp => {
            if lips {
                out.push(Op::fill(lower_lip(mx, my, 13.0, 5.5), lip));
                out.push(Op::fill(upper_lip(mx, my, 14.0, 6.0), lip.darken(0.08)));
                out.push(Op::stroke(Path::polyline(&[(mx - 12.0, my), (mx + 12.0, my)], false), MOUTH.with_alpha(0.7), 1.6));
            } else {
                let mut p = Path::new();
                p.move_to(mx - 12.0, my).quad_to(mx, my + 2.5, mx + 12.0, my);
                out.push(Op::stroke(p, MOUTH, 4.5));
                // Lips pressed together push the lower lip out a little.
                let mut q = Path::new();
                q.move_to(mx - 6.0, my + 5.0).quad_to(mx, my + 7.0, mx + 6.0, my + 5.0);
                out.push(Op::stroke(q, look.skin.darken(0.22), 2.0));
            }
        }
        Viseme::Fv => {
            // Top teeth resting on the lower lip.
            out.push(Op::fill(Path::round_rect(mx - 11.0, my - 3.5, 22.0, 7.0, 3.0), MOUTH));
            out.push(Op::fill(Path::round_rect(mx - 9.0, my - 3.0, 18.0, 4.5, 1.8), TEETH));
            if lips {
                out.push(Op::fill(upper_lip(mx, my - 3.0, 13.5, 5.0), lip.darken(0.08)));
                out.push(Op::fill(lower_lip(mx, my + 0.5, 12.0, 4.5), lip));
            } else {
                let mut p = Path::new();
                p.move_to(mx - 11.0, my + 3.5).quad_to(mx, my + 0.5, mx + 11.0, my + 3.5);
                out.push(Op::stroke(p, look.skin.darken(0.3), 2.5));
            }
        }
        _ => {
            let Some(o) = opening(v) else { return };
            if lips {
                let cy = my + o.dy;
                let (lx, ly) = (o.rx + 4.5, o.ry + 4.0);
                if o.clenched {
                    out.push(Op::fill(Path::round_rect(mx - lx, cy - ly, lx * 2.0, ly * 2.0, ly), lip));
                } else {
                    out.push(Op::fill(Path::ellipse(mx, cy, lx, ly), lip));
                }
                // A soft highlight on the lower lip.
                out.push(Op::fill(Path::ellipse(mx + 2.0, cy + ly - 2.2, lx * 0.35, 1.4), Color::rgba(255, 255, 255, 70)));
            } else if v == Viseme::U {
                // Pursed lips: a ring around the small opening.
                out.push(Op::stroke(Path::ellipse(mx, my + o.dy, o.rx + 2.5, o.ry + 2.5), look.skin.darken(0.28), 3.0));
            }
            draw_opening(out, &o, mx, my);
        }
    }
}

/// The lips set's mouth at rest, following the expression.
pub fn rest_lips(out: &mut Vec<Op>, look: &Look, e: Expression, mx: f32, my: f32) {
    use Expression::*;
    let lip = lip_color(look);
    match e {
        Neutral | Thinking => {
            let x = if e == Thinking { mx + 4.0 } else { mx };
            out.push(Op::fill(lower_lip(x, my, 10.0, 4.0), lip));
            out.push(Op::fill(upper_lip(x, my, 11.0, 4.5), lip.darken(0.08)));
            out.push(Op::stroke(Path::polyline(&[(x - 9.0, my), (x + 9.0, my)], false), MOUTH.with_alpha(0.6), 1.4));
        }
        Happy | Wink => {
            let mut p = Path::new();
            p.move_to(mx - 15.0, my - 4.0).quad_to(mx, my - 6.0, mx + 15.0, my - 4.0).quad_to(mx, my + 16.0, mx - 15.0, my - 4.0).close();
            out.push(Op::stroke(p.clone(), lip, 4.0));
            out.push(Op::fill(p, MOUTH));
            out.push(Op::fill(Path::ellipse(mx, my - 2.0, 9.0, 2.6), TEETH));
        }
        Surprised => {
            out.push(Op::fill(Path::ellipse(mx, my + 2.0, 10.0, 12.0), lip));
            out.push(Op::fill(Path::ellipse(mx, my + 2.0, 6.0, 8.0), MOUTH));
        }
        Sad | Angry => {
            let mut p = Path::new();
            let (dy, up) = if e == Sad { (5.0, -5.0) } else { (3.0, -2.0) };
            p.move_to(mx - 11.0, my + dy).quad_to(mx, my + up, mx + 11.0, my + dy);
            out.push(Op::stroke(p, lip, 5.0));
        }
        Worried | Confused => out.push(Op::stroke(crate::rig::wavy_mouth(mx, my + 2.0, 12.0), lip, 4.5)),
    }
}

/// A stick figure's line-drawn mouth shapes, in the figure's ink colour.
pub fn draw_stick(out: &mut Vec<Op>, v: Viseme, mx: f32, my: f32, ink: Color) {
    match v {
        Viseme::Rest => {}
        Viseme::Mbp => out.push(Op::stroke(Path::polyline(&[(mx - 9.0, my), (mx + 9.0, my)], false), ink, 3.5)),
        Viseme::Ee => out.push(Op::fill(Path::round_rect(mx - 9.0, my - 2.6, 18.0, 5.2, 2.6), ink)),
        Viseme::Eh => out.push(Op::fill(Path::ellipse(mx, my + 1.0, 8.0, 5.5), ink)),
        Viseme::Ai => {
            out.push(Op::fill(Path::ellipse(mx, my + 2.5, 9.0, 9.0), ink));
            out.push(Op::fill(Path::ellipse(mx, my + 7.0, 5.0, 2.6), TONGUE));
        }
        Viseme::O => out.push(Op::stroke(Path::ellipse(mx, my + 2.0, 6.0, 7.5), ink, 3.0)),
        Viseme::U => out.push(Op::stroke(Path::ellipse(mx, my + 1.0, 3.5, 4.0), ink, 3.0)),
        Viseme::Fv => {
            out.push(Op::stroke(Path::polyline(&[(mx - 8.0, my), (mx + 8.0, my)], false), ink, 3.0));
            let mut p = Path::new();
            p.move_to(mx - 5.0, my + 1.0).quad_to(mx, my + 5.0, mx + 5.0, my + 1.0);
            out.push(Op::stroke(p, ink, 2.5));
        }
        Viseme::L => {
            out.push(Op::fill(Path::ellipse(mx, my + 1.5, 8.0, 7.0), ink));
            out.push(Op::fill(Path::ellipse(mx, my - 1.5, 4.5, 2.6), TONGUE));
        }
    }
}
