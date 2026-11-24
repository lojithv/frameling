//! Asset packs: JSON bundles of props, backgrounds and character actions.
//!
//! Packs are the main customisation point: drop a `pack.json` in and new
//! content shows up in the library. Shapes are simple vector primitives or SVG
//! path data, and colours can refer to per-item colour slots (`$0`, `$1d`, ...).

use crate::display::{Op, Paint, Path};
use crate::geom::{Color, Rect};
use crate::rig::Pose;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// File format
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub props: Vec<PropDef>,
    #[serde(default)]
    pub backgrounds: Vec<BackgroundDef>,
    #[serde(default)]
    pub actions: Vec<ActionDef>,
    /// Character parts (wearables) for the character creator.
    #[serde(default)]
    pub parts: Vec<PartDef>,
    /// Reusable poses (the pose library).
    #[serde(default)]
    pub poses: Vec<PoseDef>,
}

/// A reusable pose: joint angles that fit every character (cartoon, stick
/// and creator characters share one skeleton).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoseDef {
    pub id: String,
    pub name: String,
    /// Standing, Gestures, Sitting and kneeling, Lying, Motion keys.
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub pose: Pose,
}

/// Where a character part attaches. Coordinates in a part are relative to
/// its anchor, on a cartoon-sized figure; stick figures scale them to fit.
///
/// | Anchor | Origin | Scale |
/// |---|---|---|
/// | `head` | centre of the head (radius 50) | the head |
/// | `neck` | base of the neck, between the shoulders | the torso width |
/// | `torso` | middle of the chest | the torso width |
/// | `handR`, `handL` | centre of the (screen) right / left hand | none |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    #[default]
    Head,
    Neck,
    Torso,
    HandR,
    HandL,
}

/// Draw a part in front of its anchor (default) or behind the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Layer {
    #[default]
    Front,
    Back,
}

/// A wearable character part: shapes in anchor space, colour slots, tags.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartDef {
    pub id: String,
    pub name: String,
    /// Creator category: headwear, eyewear, neckwear, earrings, held, back, face.
    pub category: String,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub layer: Layer,
    /// Default colours for the `$0`, `$1`, ... slots.
    #[serde(default)]
    pub colors: Vec<Color>,
    /// Words agents and the search box match ("winter", "music", "formal").
    #[serde(default)]
    pub tags: Vec<String>,
    /// Figure styles it fits ("cartoon", "stick"); empty = all.
    #[serde(default)]
    pub styles: Vec<String>,
    /// Hide the hair in front (tight hats).
    #[serde(default)]
    pub hides_hair: bool,
    pub shapes: Vec<ShapeDef>,
}

/// A compiled part.
#[derive(Clone, Debug)]
pub struct Part {
    pub def: PartDef,
    pub drawing: Drawing,
}

impl Part {
    pub fn compile(def: &PartDef) -> Result<Part, String> {
        if def.id.is_empty() {
            return Err("part id must not be empty".into());
        }
        let drawing = Drawing::compile(&def.id, &def.name, &def.category, &def.colors, &def.shapes)?;
        Ok(Part { def: def.clone(), drawing })
    }
    pub fn fits(&self, style: &str) -> bool {
        self.def.styles.is_empty() || self.def.styles.iter().any(|s| s == style)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub colors: Vec<Color>,
    pub shapes: Vec<ShapeDef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub colors: Vec<Color>,
    pub shapes: Vec<ShapeDef>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDef {
    pub id: String,
    pub name: String,
    /// Seconds for one cycle.
    pub duration: f32,
    #[serde(default = "yes")]
    pub r#loop: bool,
    pub keys: Vec<ActionKey>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ActionKey {
    /// Time as a fraction of the cycle, 0..1.
    pub t: f32,
    pub pose: Pose,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ShapeDef {
    #[serde(flatten)]
    pub geom: Geom,
    #[serde(default)]
    pub fill: Option<FillDef>,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default = "one")]
    pub sw: f32,
    #[serde(default = "one")]
    pub opacity: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Geom {
    Rect { x: f32, y: f32, w: f32, h: f32, #[serde(default)] r: f32 },
    Circle { cx: f32, cy: f32, r: f32 },
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32 },
    Poly { points: Vec<[f32; 2]> },
    Line { x1: f32, y1: f32, x2: f32, y2: f32 },
    Path { d: String },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FillDef {
    Color(String),
    Linear { lin: [f32; 4], stops: Vec<String> },
}

// ---------------------------------------------------------------------------
// Compiled form
// ---------------------------------------------------------------------------

/// A colour that is either fixed or taken from an item's colour slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorRef {
    Fixed(Color),
    /// Slot index plus a shade: negative darkens, positive lightens.
    Slot(usize, f32),
}

impl ColorRef {
    fn parse(s: &str) -> Result<ColorRef, String> {
        if let Some(rest) = s.strip_prefix('$') {
            let (num, shade) = match rest.find(|c: char| !c.is_ascii_digit()) {
                Some(i) => (&rest[..i], &rest[i..]),
                None => (rest, ""),
            };
            let idx: usize = num.parse().map_err(|_| format!("bad colour slot {s:?}"))?;
            let shade = match shade {
                "" => 0.0,
                "d" => -0.2,
                "dd" => -0.4,
                "l" => 0.25,
                "ll" => 0.5,
                _ => return Err(format!("bad colour shade in {s:?}")),
            };
            Ok(ColorRef::Slot(idx, shade))
        } else {
            Color::parse(s).map(ColorRef::Fixed).ok_or_else(|| format!("bad colour {s:?}"))
        }
    }
    pub fn resolve(self, slots: &[Color]) -> Color {
        match self {
            ColorRef::Fixed(c) => c,
            ColorRef::Slot(i, shade) => {
                let c = slots.get(i).copied().unwrap_or(Color::rgb(0xff, 0x00, 0xff));
                if shade < 0.0 {
                    c.darken(-shade)
                } else if shade > 0.0 {
                    c.lighten(shade)
                } else {
                    c
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
enum FillC {
    Solid(ColorRef),
    Linear([f32; 4], Vec<ColorRef>),
}

#[derive(Clone, Debug)]
pub struct Shape {
    path: Path,
    fill: Option<FillC>,
    stroke: Option<(ColorRef, f32)>,
    opacity: f32,
}

/// A drawable made of shapes (props and backgrounds).
#[derive(Clone, Debug)]
pub struct Drawing {
    pub id: String,
    pub name: String,
    pub category: String,
    pub colors: Vec<Color>,
    pub bounds: Rect,
    shapes: Vec<Shape>,
}

impl Drawing {
    fn compile(id: &str, name: &str, category: &str, colors: &[Color], defs: &[ShapeDef]) -> Result<Drawing, String> {
        let mut shapes = Vec::with_capacity(defs.len());
        let mut bounds = Rect::empty();
        for d in defs {
            let path = match &d.geom {
                Geom::Rect { x, y, w, h, r } => Path::round_rect(*x, *y, *w, *h, *r),
                Geom::Circle { cx, cy, r } => Path::circle(*cx, *cy, *r),
                Geom::Ellipse { cx, cy, rx, ry } => Path::ellipse(*cx, *cy, *rx, *ry),
                Geom::Poly { points } => {
                    Path::polyline(&points.iter().map(|p| (p[0], p[1])).collect::<Vec<_>>(), true)
                }
                Geom::Line { x1, y1, x2, y2 } => Path::polyline(&[(*x1, *y1), (*x2, *y2)], false),
                Geom::Path { d } => Path::parse_svg(d).map_err(|e| format!("{id}: {e}"))?,
            };
            let fill = match &d.fill {
                None => None,
                Some(FillDef::Color(c)) => Some(FillC::Solid(ColorRef::parse(c)?)),
                Some(FillDef::Linear { lin, stops }) => {
                    Some(FillC::Linear(*lin, stops.iter().map(|s| ColorRef::parse(s)).collect::<Result<_, _>>()?))
                }
            };
            let stroke = d.stroke.as_deref().map(ColorRef::parse).transpose()?.map(|c| (c, d.sw));
            let b = path.bounds();
            if !b.is_empty() {
                bounds.include(b.x, b.y);
                bounds.include(b.x + b.w, b.y + b.h);
            }
            shapes.push(Shape { path, fill, stroke, opacity: d.opacity });
        }
        Ok(Drawing { id: id.into(), name: name.into(), category: category.into(), colors: colors.to_vec(), bounds, shapes })
    }

    /// Draw ops with colour slot overrides applied.
    pub fn ops(&self, overrides: &[Color]) -> Vec<Op> {
        let slots: Vec<Color> =
            (0..self.colors.len()).map(|i| overrides.get(i).copied().unwrap_or(self.colors[i])).collect();
        let mut out = Vec::with_capacity(self.shapes.len());
        for s in &self.shapes {
            let alpha = s.opacity;
            if let Some(f) = &s.fill {
                let paint = match f {
                    FillC::Solid(c) => Paint::solid(c.resolve(&slots).with_alpha(alpha)),
                    FillC::Linear(l, stops) => {
                        let n = stops.len().max(2) - 1;
                        Paint::Linear {
                            x0: l[0],
                            y0: l[1],
                            x1: l[2],
                            y1: l[3],
                            stops: stops
                                .iter()
                                .enumerate()
                                .map(|(i, c)| (i as f32 / n as f32, c.resolve(&slots).with_alpha(alpha)))
                                .collect(),
                        }
                    }
                };
                out.push(Op::Fill { path: s.path.clone(), paint });
            }
            if let Some((c, w)) = s.stroke {
                out.push(Op::stroke(s.path.clone(), c.resolve(&slots).with_alpha(alpha), w));
            }
        }
        out
    }
}

/// Info about library entries, for the UI.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub colors: Vec<Color>,
    pub pack: String,
}

/// Everything the evaluator can draw: the union of all loaded packs.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub props: BTreeMap<String, Drawing>,
    pub backgrounds: BTreeMap<String, Drawing>,
    pub actions: BTreeMap<String, ActionDef>,
    pub parts: BTreeMap<String, Part>,
    pub poses: BTreeMap<String, PoseDef>,
    /// Which pack each entry came from: (kind, id) -> pack id.
    origin: BTreeMap<(u8, String), String>,
    pub packs: Vec<(String, String)>,
}

pub const CORE_PACK_JSON: &str = include_str!("../../../packs/core/pack.json");
/// Stick-figure props, backgrounds and extra poses (generated by tools/build_stick_pack.mjs).
pub const STICK_PACK_JSON: &str = include_str!("../../../packs/stick/pack.json");
/// More rooms, outdoor scenes and props (generated by tools/build_scenes_pack.mjs).
pub const SCENES_PACK_JSON: &str = include_str!("../../../packs/scenes/pack.json");
/// The built-in pose library.
pub const POSES_PACK_JSON: &str = include_str!("../../../packs/poses/pack.json");
/// Built-in wearable parts for the character creator.
pub const PARTS_PACK_JSON: &str = include_str!("../../../packs/parts/pack.json");

impl Library {
    /// The built-in library (core pack), parsed once.
    pub fn builtin() -> &'static Library {
        static LIB: OnceLock<Library> = OnceLock::new();
        LIB.get_or_init(|| {
            let mut l = Library::default();
            l.add_pack_json(CORE_PACK_JSON).expect("core pack is valid");
            l.add_pack_json(STICK_PACK_JSON).expect("stick pack is valid");
            l.add_pack_json(SCENES_PACK_JSON).expect("scenes pack is valid");
            l.add_pack_json(PARTS_PACK_JSON).expect("parts pack is valid");
            l.add_pack_json(POSES_PACK_JSON).expect("poses pack is valid");
            // Motion keys straight from the walk cycle, so they always match it.
            for (id, name, pose) in [
                ("walk-stride", "Walk stride", crate::rig::locomotion(0.04, false, 1.0)),
                ("passing", "Walk passing", crate::rig::locomotion(0.29, false, 1.0)),
                ("run-stride", "Run stride", crate::rig::locomotion(0.12, true, 1.0)),
            ] {
                let tags = vec!["walk".to_string(), "run".into(), "step".into(), "motion".into()];
                l.poses.insert(id.into(), PoseDef { id: id.into(), name: name.into(), category: "Motion keys".into(), tags, pose });
                l.origin.insert((4, id.to_string()), "poses".into());
            }
            l
        })
    }

    /// Load a pack. Later packs override entries with the same id.
    pub fn add_pack_json(&mut self, json: &str) -> Result<String, String> {
        let pack: PackFile = serde_json::from_str(json).map_err(|e| format!("invalid pack: {e}"))?;
        self.add_pack(pack)
    }

    pub fn add_pack(&mut self, pack: PackFile) -> Result<String, String> {
        for p in &pack.props {
            let d = Drawing::compile(&p.id, &p.name, &p.category, &p.colors, &p.shapes)?;
            self.origin.insert((0, p.id.clone()), pack.id.clone());
            self.props.insert(p.id.clone(), d);
        }
        for b in &pack.backgrounds {
            let d = Drawing::compile(&b.id, &b.name, "", &b.colors, &b.shapes)?;
            self.origin.insert((1, b.id.clone()), pack.id.clone());
            self.backgrounds.insert(b.id.clone(), d);
        }
        for a in &pack.actions {
            if a.keys.is_empty() {
                return Err(format!("action {} has no keys", a.id));
            }
            self.origin.insert((2, a.id.clone()), pack.id.clone());
            let mut a = a.clone();
            a.keys.sort_by(|x, y| x.t.total_cmp(&y.t));
            self.actions.insert(a.id.clone(), a);
        }
        for p in &pack.parts {
            let part = Part::compile(p)?;
            self.origin.insert((3, p.id.clone()), pack.id.clone());
            self.parts.insert(p.id.clone(), part);
        }
        for p in &pack.poses {
            self.origin.insert((4, p.id.clone()), pack.id.clone());
            self.poses.insert(p.id.clone(), p.clone());
        }
        self.packs.retain(|(id, _)| id != &pack.id);
        self.packs.push((pack.id.clone(), pack.name.clone()));
        Ok(pack.id)
    }

    fn entries<'a>(&'a self, kind: u8, it: impl Iterator<Item = (&'a String, &'a Drawing)>) -> Vec<Entry> {
        it.map(|(id, d)| Entry {
            id: id.clone(),
            name: d.name.clone(),
            category: d.category.clone(),
            colors: d.colors.clone(),
            pack: self.origin.get(&(kind, id.clone())).cloned().unwrap_or_default(),
        })
        .collect()
    }

    /// The pack a part came from.
    pub fn part_pack(&self, id: &str) -> String {
        self.origin.get(&(3, id.to_string())).cloned().unwrap_or_default()
    }

    /// JSON catalogue of the library for the UI.
    pub fn catalog(&self) -> serde_json::Value {
        let actions: Vec<_> = self
            .actions
            .values()
            .map(|a| serde_json::json!({ "id": a.id, "name": a.name, "loop": a.r#loop, "duration": a.duration }))
            .collect();
        serde_json::json!({
            "props": self.entries(0, self.props.iter()),
            "backgrounds": self.entries(1, self.backgrounds.iter()),
            "actions": actions,
            "parts": crate::parts::catalog(self, None),
            "poses": self.poses.values().map(|p| serde_json::json!({ "id": p.id, "name": p.name, "category": p.category, "tags": p.tags, "pack": self.origin.get(&(4, p.id.clone())).cloned().unwrap_or_default() })).collect::<Vec<_>>(),
            "packs": self.packs.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
        })
    }
}
