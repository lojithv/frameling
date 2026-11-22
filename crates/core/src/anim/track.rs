//! Keys, interpolation and sampling.
//!
//! A track is a list of keys sorted by frame. Each key's `interp` shapes the
//! segment from that key to the next one (the convention Blender and After
//! Effects use). Before the first key a track holds the first value; after the
//! last key it holds the last value.
//!
//! Frames are integers in the document, but sampling takes an `f32` so callers
//! can look between frames (auto-walk uses this to find the direction of travel).

use crate::geom::Color;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How a segment moves from its key to the next one.
///
/// JSON: a string (`"linear"`, `"constant"`, `"easeIn"`, `"easeOut"`,
/// `"easeInOut"`, `"back"`, `"elastic"`, `"bounce"`) or `{"bezier":[x1,y1,x2,y2]}`
/// with the same meaning as CSS `cubic-bezier()`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Interp {
    /// Hold this key's value until the next key (a jump cut).
    #[serde(alias = "step", alias = "hold")]
    Constant,
    #[default]
    Linear,
    #[serde(alias = "in")]
    EaseIn,
    #[serde(alias = "out")]
    EaseOut,
    #[serde(alias = "inOut", alias = "smooth")]
    EaseInOut,
    /// Overshoots the target slightly, then settles.
    Back,
    /// Springs past the target and wobbles into place.
    Elastic,
    /// Lands and bounces a few times.
    Bounce,
    /// CSS-style cubic bezier: control points (x1, y1) and (x2, y2), x in 0..=1.
    Bezier([f32; 4]),
}

impl Interp {
    /// Map linear progress `t` (0..=1) through the curve. Results may leave
    /// 0..=1 for back, elastic and steep beziers (overshoot is intended).
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Interp::Constant => 0.0,
            Interp::Linear => t,
            Interp::EaseIn => t * t * t,
            Interp::EaseOut => 1.0 - (1.0 - t).powi(3),
            Interp::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Interp::Back => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Interp::Elastic => {
                if t <= 0.0 || t >= 1.0 {
                    t
                } else {
                    let c4 = std::f32::consts::TAU / 3.0;
                    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
            Interp::Bounce => bounce_out(t),
            Interp::Bezier([x1, y1, x2, y2]) => cubic_bezier(x1.clamp(0.0, 1.0), y1, x2.clamp(0.0, 1.0), y2, t),
        }
    }

    /// Whether values may be blended between keys (false = step).
    pub fn blends(self) -> bool {
        self != Interp::Constant
    }
}

fn bounce_out(t: f32) -> f32 {
    let (n1, d1) = (7.5625, 2.75);
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

/// Evaluate a CSS `cubic-bezier(x1, y1, x2, y2)` at progress `x`.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    // Polynomial coefficients for one axis of a bezier from 0 to 1.
    let coeff = |p1: f32, p2: f32| {
        let c = 3.0 * p1;
        let b = 3.0 * (p2 - p1) - c;
        let a = 1.0 - c - b;
        (a, b, c)
    };
    let (ax, bx, cx) = coeff(x1, x2);
    let (ay, by, cy) = coeff(y1, y2);
    let sx = |s: f32| ((ax * s + bx) * s + cx) * s;
    let dsx = |s: f32| (3.0 * ax * s + 2.0 * bx) * s + cx;
    let sy = |s: f32| ((ay * s + by) * s + cy) * s;
    // Newton's method, then bisection if the slope is too flat.
    let mut s = x;
    for _ in 0..8 {
        let err = sx(s) - x;
        if err.abs() < 1e-6 {
            return sy(s);
        }
        let d = dsx(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= err / d;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    s = x;
    for _ in 0..40 {
        let v = sx(s);
        if (v - x).abs() < 1e-6 {
            break;
        }
        if v < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    sy(s)
}

/// One key on a track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    /// Scene-local frame.
    pub frame: u32,
    /// Number, `"#rrggbb"` colour, bool, enum name or outfit, depending on the path.
    pub value: Value,
    /// Shape of the segment from this key to the next one.
    #[serde(default, skip_serializing_if = "is_linear")]
    pub interp: Interp,
}

fn is_linear(i: &Interp) -> bool {
    *i == Interp::Linear
}

impl Key {
    pub fn new(frame: u32, value: impl Into<Value>) -> Key {
        Key { frame, value: value.into(), interp: Interp::Linear }
    }
    pub fn with(mut self, interp: Interp) -> Key {
        self.interp = interp;
        self
    }
}

/// Keys for one property, sorted by frame, at most one key per frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Track {
    pub keys: Vec<Key>,
}

/// Where a frame falls on a track: the keys on each side and the eased weight
/// of the second one (0 = all `a`, 1 = all `b`).
pub struct Span<'a> {
    pub a: &'a Key,
    pub b: &'a Key,
    pub weight: f32,
}

impl Track {
    /// Insert or replace keys (a key at an existing frame replaces it).
    pub fn upsert(&mut self, keys: impl IntoIterator<Item = Key>) {
        for k in keys {
            match self.keys.binary_search_by_key(&k.frame, |e| e.frame) {
                Ok(i) => self.keys[i] = k,
                Err(i) => self.keys.insert(i, k),
            }
        }
    }

    /// Restore the sorted, unique invariant (after loading hand-written files).
    pub fn normalize(&mut self) {
        self.keys.sort_by_key(|k| k.frame);
        // Keep the last key written for a frame.
        let mut out: Vec<Key> = Vec::with_capacity(self.keys.len());
        for k in self.keys.drain(..) {
            match out.last_mut() {
                Some(l) if l.frame == k.frame => *l = k,
                _ => out.push(k),
            }
        }
        self.keys = out;
    }

    /// Find the keys around `frame` (binary search). `None` for an empty track.
    pub fn span(&self, frame: f32) -> Option<Span<'_>> {
        let keys = &self.keys;
        let first = keys.first()?;
        if keys.len() == 1 || frame <= first.frame as f32 {
            return Some(Span { a: first, b: first, weight: 0.0 });
        }
        // Index of the first key strictly after `frame`.
        let i = keys.partition_point(|k| k.frame as f32 <= frame);
        if i >= keys.len() {
            let last = keys.last().unwrap();
            return Some(Span { a: last, b: last, weight: 0.0 });
        }
        let (a, b) = (&keys[i - 1], &keys[i]);
        let t = (frame - a.frame as f32) / (b.frame - a.frame) as f32;
        Some(Span { a, b, weight: a.interp.apply(t) })
    }

    /// Sample as a number (non-numeric keys read as 0).
    pub fn number(&self, frame: f32) -> Option<f32> {
        let s = self.span(frame)?;
        let (a, b) = (num(&s.a.value), num(&s.b.value));
        Some(a + (b - a) * s.weight)
    }

    /// Sample as a colour; blends in RGB unless the segment is constant.
    pub fn color(&self, frame: f32) -> Option<Color> {
        let s = self.span(frame)?;
        let a = color(&s.a.value)?;
        if !s.a.interp.blends() || std::ptr::eq(s.a, s.b) {
            return Some(a);
        }
        let b = color(&s.b.value).unwrap_or(a);
        Some(a.mix(b, s.weight.clamp(0.0, 1.0)))
    }

    /// Sample a value that can't be blended (enums, bools, outfits): it
    /// switches exactly on each key's frame.
    pub fn step(&self, frame: f32) -> Option<&Value> {
        let s = self.span(frame)?;
        Some(&s.a.value)
    }
}

pub fn num(v: &Value) -> f32 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0) as f32,
        Value::Bool(b) => *b as u8 as f32,
        _ => 0.0,
    }
}

pub fn color(v: &Value) -> Option<Color> {
    v.as_str().and_then(Color::parse)
}
