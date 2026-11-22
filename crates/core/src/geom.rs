//! Small geometry and colour primitives shared by the evaluator and renderers.

use serde::{Deserialize, Serialize};

/// 2D affine transform `[a c e; b d f; 0 0 1]` (same layout as Canvas2D / SVG).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn translate(x: f32, y: f32) -> Self {
        Affine { e: x, f: y, ..Self::IDENTITY }
    }
    pub fn scale(sx: f32, sy: f32) -> Self {
        Affine { a: sx, d: sy, ..Self::IDENTITY }
    }
    /// Rotation in degrees (positive = clockwise on screen, because y points down).
    pub fn rotate_deg(deg: f32) -> Self {
        let r = deg.to_radians();
        let (s, c) = r.sin_cos();
        Affine { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }
    /// `self * other`: apply `other` first, then `self`.
    pub fn then(&self, other: &Affine) -> Affine {
        let m = self;
        let n = other;
        Affine {
            a: m.a * n.a + m.c * n.b,
            b: m.b * n.a + m.d * n.b,
            c: m.a * n.c + m.c * n.d,
            d: m.b * n.c + m.d * n.d,
            e: m.a * n.e + m.c * n.f + m.e,
            f: m.b * n.e + m.d * n.f + m.f,
        }
    }
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }
}

/// RGBA colour, 0-255 per channel. Serialised as `"#rrggbb"` or `"#rrggbbaa"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const INK: Color = Color::rgb(0x1f, 0x22, 0x33);

    pub fn with_alpha(self, a: f32) -> Color {
        Color { a: (self.a as f32 * a.clamp(0.0, 1.0)).round() as u8, ..self }
    }
    /// Mix towards `other` by `t` (0 = self, 1 = other).
    pub fn mix(self, other: Color, t: f32) -> Color {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
        Color { r: l(self.r, other.r), g: l(self.g, other.g), b: l(self.b, other.b), a: l(self.a, other.a) }
    }
    pub fn darken(self, t: f32) -> Color {
        self.mix(Color { a: self.a, ..Color::BLACK }, t)
    }
    pub fn lighten(self, t: f32) -> Color {
        self.mix(Color { a: self.a, ..Color::WHITE }, t)
    }

    pub fn parse(s: &str) -> Option<Color> {
        let h = s.trim().strip_prefix('#')?;
        let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            3 => {
                let n = |i: usize| u8::from_str_radix(h.get(i..i + 1)?, 16).ok().map(|v| v * 17);
                Some(Color::rgb(n(0)?, n(1)?, n(2)?))
            }
            6 => Some(Color::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Color::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("bad colour {s:?}")))
    }
}

/// Axis-aligned rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn empty() -> Rect {
        Rect { x: f32::INFINITY, y: f32::INFINITY, w: f32::NEG_INFINITY, h: f32::NEG_INFINITY }
    }
    pub fn is_empty(&self) -> bool {
        !(self.w.is_finite() && self.h.is_finite()) || self.w < 0.0 || self.h < 0.0
    }
    pub fn include(&mut self, x: f32, y: f32) {
        if self.is_empty() {
            *self = Rect { x, y, w: 0.0, h: 0.0 };
            return;
        }
        let x0 = self.x.min(x);
        let y0 = self.y.min(y);
        let x1 = (self.x + self.w).max(x);
        let y1 = (self.y + self.h).max(y);
        *self = Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    }
    /// Bounding box of this rect after a transform.
    pub fn transformed(&self, t: &Affine) -> Rect {
        let mut r = Rect::empty();
        for (x, y) in [(self.x, self.y), (self.x + self.w, self.y), (self.x, self.y + self.h), (self.x + self.w, self.y + self.h)] {
            let (px, py) = t.apply(x, y);
            r.include(px, py);
        }
        r
    }
}

/// Easing curves used by motion, camera moves and keyframes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Ease {
    Linear,
    #[default]
    InOut,
    In,
    Out,
    Back,
}

impl Ease {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::In => t * t * t,
            Ease::Out => 1.0 - (1.0 - t).powi(3),
            Ease::InOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Ease::Back => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Deterministic 32-bit hash for strings (FNV-1a). Used to give each item its
/// own blink timing etc. without any randomness at render time.
pub fn hash_str(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}
