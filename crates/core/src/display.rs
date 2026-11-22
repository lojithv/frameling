//! The display list: a flat-ish tree of vector draw operations.
//!
//! `evaluate()` turns a project + frame number into a `DisplayList`. Every
//! renderer (CPU tiny-skia today, GPU later) only has to understand these few
//! ops, which is what keeps preview and export pixel-identical.

use crate::geom::{Affine, Color, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k")]
pub enum Seg {
    M { x: f32, y: f32 },
    L { x: f32, y: f32 },
    Q { x1: f32, y1: f32, x: f32, y: f32 },
    C { x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32 },
    Z,
}

/// A vector path made of move/line/quad/cubic segments.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Path(pub Vec<Seg>);

const KAPPA: f32 = 0.552_284_8;

impl Path {
    pub fn new() -> Self {
        Path(Vec::new())
    }
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.0.push(Seg::M { x, y });
        self
    }
    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.0.push(Seg::L { x, y });
        self
    }
    pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) -> &mut Self {
        self.0.push(Seg::Q { x1, y1, x, y });
        self
    }
    pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) -> &mut Self {
        self.0.push(Seg::C { x1, y1, x2, y2, x, y });
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.0.push(Seg::Z);
        self
    }
    pub fn extend(&mut self, other: &Path) {
        self.0.extend_from_slice(&other.0);
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
        let mut p = Path::new();
        p.move_to(x, y).line_to(x + w, y).line_to(x + w, y + h).line_to(x, y + h).close();
        p
    }

    pub fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        if r <= 0.0 {
            return Path::rect(x, y, w, h);
        }
        let k = r * (1.0 - KAPPA);
        let mut p = Path::new();
        p.move_to(x + r, y)
            .line_to(x + w - r, y)
            .cubic_to(x + w - k, y, x + w, y + k, x + w, y + r)
            .line_to(x + w, y + h - r)
            .cubic_to(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h)
            .line_to(x + r, y + h)
            .cubic_to(x + k, y + h, x, y + h - k, x, y + h - r)
            .line_to(x, y + r)
            .cubic_to(x, y + k, x + k, y, x + r, y)
            .close();
        p
    }

    pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        let mut p = Path::new();
        p.move_to(cx + rx, cy)
            .cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry)
            .cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy)
            .cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry)
            .cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy)
            .close();
        p
    }

    pub fn circle(cx: f32, cy: f32, r: f32) -> Path {
        Path::ellipse(cx, cy, r, r)
    }

    pub fn polyline(points: &[(f32, f32)], closed: bool) -> Path {
        let mut p = Path::new();
        for (i, &(x, y)) in points.iter().enumerate() {
            if i == 0 {
                p.move_to(x, y);
            } else {
                p.line_to(x, y);
            }
        }
        if closed {
            p.close();
        }
        p
    }

    /// Control-point bounding box (a cheap, conservative bound).
    pub fn bounds(&self) -> Rect {
        let mut r = Rect::empty();
        for s in &self.0 {
            match *s {
                Seg::M { x, y } | Seg::L { x, y } => r.include(x, y),
                Seg::Q { x1, y1, x, y } => {
                    r.include(x1, y1);
                    r.include(x, y);
                }
                Seg::C { x1, y1, x2, y2, x, y } => {
                    r.include(x1, y1);
                    r.include(x2, y2);
                    r.include(x, y);
                }
                Seg::Z => {}
            }
        }
        r
    }

    /// Parse the subset of SVG path data used by asset packs:
    /// M/L/H/V/C/S/Q/T/Z in absolute and relative forms.
    pub fn parse_svg(d: &str) -> Result<Path, String> {
        let mut toks = Tokenizer { s: d.as_bytes(), i: 0 };
        let mut p = Path::new();
        let (mut cx, mut cy) = (0.0f32, 0.0f32);
        let (mut sx, mut sy) = (0.0f32, 0.0f32);
        let mut last_ctrl: Option<(f32, f32)> = None;
        let mut cmd = b'M';
        loop {
            toks.skip_ws();
            if toks.done() {
                break;
            }
            if let Some(c) = toks.peek_cmd() {
                cmd = c;
                toks.i += 1;
            }
            let rel = cmd.is_ascii_lowercase();
            let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
            match cmd.to_ascii_uppercase() {
                b'M' => {
                    let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                    p.move_to(x, y);
                    (cx, cy, sx, sy) = (x, y, x, y);
                    cmd = if rel { b'l' } else { b'L' };
                    last_ctrl = None;
                }
                b'L' => {
                    let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                    p.line_to(x, y);
                    (cx, cy) = (x, y);
                    last_ctrl = None;
                }
                b'H' => {
                    let x = toks.num()? + ox;
                    p.line_to(x, cy);
                    cx = x;
                    last_ctrl = None;
                }
                b'V' => {
                    let y = toks.num()? + oy;
                    p.line_to(cx, y);
                    cy = y;
                    last_ctrl = None;
                }
                b'C' => {
                    let (x1, y1) = (toks.num()? + ox, toks.num()? + oy);
                    let (x2, y2) = (toks.num()? + ox, toks.num()? + oy);
                    let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                    p.cubic_to(x1, y1, x2, y2, x, y);
                    last_ctrl = Some((x2, y2));
                    (cx, cy) = (x, y);
                }
                b'S' => {
                    let (x1, y1) = match last_ctrl {
                        Some((lx, ly)) => (2.0 * cx - lx, 2.0 * cy - ly),
                        None => (cx, cy),
                    };
                    let (x2, y2) = (toks.num()? + ox, toks.num()? + oy);
                    let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                    p.cubic_to(x1, y1, x2, y2, x, y);
                    last_ctrl = Some((x2, y2));
                    (cx, cy) = (x, y);
                }
                b'Q' => {
                    let (x1, y1) = (toks.num()? + ox, toks.num()? + oy);
                    let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                    p.quad_to(x1, y1, x, y);
                    last_ctrl = Some((x1, y1));
                    (cx, cy) = (x, y);
                }
                b'T' => {
                    let (x1, y1) = match last_ctrl {
                        Some((lx, ly)) => (2.0 * cx - lx, 2.0 * cy - ly),
                        None => (cx, cy),
                    };
                    let (x, y) = (toks.num()? + ox, toks.num()? + oy);
                    p.quad_to(x1, y1, x, y);
                    last_ctrl = Some((x1, y1));
                    (cx, cy) = (x, y);
                }
                b'Z' => {
                    p.close();
                    (cx, cy) = (sx, sy);
                    last_ctrl = None;
                }
                other => return Err(format!("unsupported path command {:?}", other as char)),
            }
        }
        Ok(p)
    }
}

struct Tokenizer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Tokenizer<'_> {
    fn done(&self) -> bool {
        self.i >= self.s.len()
    }
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',') {
            self.i += 1;
        }
    }
    fn peek_cmd(&self) -> Option<u8> {
        let c = *self.s.get(self.i)?;
        (c.is_ascii_alphabetic() && c != b'e' && c != b'E').then_some(c)
    }
    fn num(&mut self) -> Result<f32, String> {
        self.skip_ws();
        let start = self.i;
        let s = self.s;
        if self.i < s.len() && (s[self.i] == b'-' || s[self.i] == b'+') {
            self.i += 1;
        }
        let mut seen_dot = false;
        while self.i < s.len() {
            let c = s[self.i];
            if c.is_ascii_digit() {
                self.i += 1;
            } else if c == b'.' && !seen_dot {
                seen_dot = true;
                self.i += 1;
            } else if (c == b'e' || c == b'E') && self.i > start {
                self.i += 1;
                if self.i < s.len() && (s[self.i] == b'-' || s[self.i] == b'+') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        std::str::from_utf8(&s[start..self.i])
            .ok()
            .and_then(|t| t.parse::<f32>().ok())
            .ok_or_else(|| format!("expected number at byte {start}"))
    }
}

/// How a shape is filled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k")]
pub enum Paint {
    Solid { color: Color },
    /// Linear gradient between two points with evenly or explicitly placed stops.
    Linear { x0: f32, y0: f32, x1: f32, y1: f32, stops: Vec<(f32, Color)> },
}

impl Paint {
    pub fn solid(c: Color) -> Paint {
        Paint::Solid { color: c }
    }
}

/// One draw operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Op {
    /// Draw `children` with an extra transform and (group) opacity.
    Group { transform: Affine, opacity: f32, children: Vec<Op> },
    Fill { path: Path, paint: Paint },
    Stroke { path: Path, color: Color, width: f32 },
}

impl Op {
    pub fn fill(path: Path, c: Color) -> Op {
        Op::Fill { path, paint: Paint::solid(c) }
    }
    pub fn stroke(path: Path, c: Color, width: f32) -> Op {
        Op::Stroke { path, color: c, width }
    }
    pub fn group(transform: Affine, opacity: f32, children: Vec<Op>) -> Op {
        Op::Group { transform, opacity, children }
    }

    /// Bounds of this op in the coordinate space it is drawn in.
    pub fn bounds(&self) -> Rect {
        match self {
            Op::Fill { path, .. } => path.bounds(),
            Op::Stroke { path, width, .. } => {
                let b = path.bounds();
                if b.is_empty() {
                    return b;
                }
                let h = width / 2.0;
                Rect { x: b.x - h, y: b.y - h, w: b.w + 2.0 * h, h: b.h + 2.0 * h }
            }
            Op::Group { transform, children, .. } => {
                let mut r = Rect::empty();
                for c in children {
                    let b = c.bounds();
                    if b.is_empty() {
                        continue;
                    }
                    let t = b.transformed(transform);
                    r.include(t.x, t.y);
                    r.include(t.x + t.w, t.y + t.h);
                }
                r
            }
        }
    }
}

/// Everything needed to draw one frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayList {
    pub width: u32,
    pub height: u32,
    pub background: Option<Color>,
    pub ops: Vec<Op>,
}
