//! Track path grammar: which property a track animates.
//!
//! One grammar is used everywhere (commands, MCP, the UI):
//!
//! | Path | Applies to | Value |
//! |---|---|---|
//! | `transform.x`, `transform.y` | any item | number, scene px |
//! | `transform.rotation` | any item | number, degrees clockwise |
//! | `transform.scale` | any item | number, replaces the item's scale |
//! | `transform.scaleX`, `transform.scaleY` | any item | number, multiplier (squash and stretch) |
//! | `transform.opacity` | any item | number 0..1 |
//! | `transform.flip` | any item | bool (steps) |
//! | `bone.<bone>.rot` (or `bone.<channel>`) | character | number, degrees |
//! | `look.skin`, `look.hair`, `look.top`, `look.bottom`, `look.shoes`, `look.accent` | character | `"#rrggbb"` |
//! | `look.hairStyle`, `look.build`, `look.accessory`, `look.style`, `look.dress` | character | enum name / bool (steps) |
//! | `outfit` | character | outfit name, `"default"`, or a partial look object (steps) |
//! | `expression` | character | expression name (steps) |
//! | `camera.x`, `camera.y`, `camera.zoom` | scene | number |
//!
//! Aliases (`x`, `bone.le`, `look.hair_style`, ...) are accepted and stored
//! under the canonical path, so one property never ends up with two tracks.

use crate::geom::Color;
use crate::model::{Accessory, Build, CharacterItem, Expression, FigureStyle, HairStyle, ItemKind, LookPatch};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// Character pose channels (see `rig::Pose`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bone {
    Rise,
    Bob,
    Lean,
    Head,
    Ls,
    Le,
    Rs,
    Re,
    Lh,
    Lk,
    Rh,
    Rk,
    Look,
    Thumb,
    Drop,
    Fold,
    Kneel,
    Tilt,
    Ground,
}

/// (canonical path, short alias, channel, what it does).
pub const BONES: &[(&str, &str, Bone, &str)] = &[
    ("bone.root.y", "bone.rise", Bone::Rise, "lift the whole body, px up"),
    ("bone.spine.y", "bone.bob", Bone::Bob, "push the upper body down, px"),
    ("bone.spine.rot", "bone.lean", Bone::Lean, "lean the upper body, degrees"),
    ("bone.head.rot", "bone.head", Bone::Head, "tilt the head, degrees"),
    ("bone.upper_arm_l.rot", "bone.ls", Bone::Ls, "screen-left shoulder, degrees from hanging down, + raises outward"),
    ("bone.forearm_l.rot", "bone.le", Bone::Le, "screen-left elbow, degrees"),
    ("bone.upper_arm_r.rot", "bone.rs", Bone::Rs, "screen-right shoulder, degrees"),
    ("bone.forearm_r.rot", "bone.re", Bone::Re, "screen-right elbow, degrees"),
    ("bone.thigh_l.rot", "bone.lh", Bone::Lh, "screen-left hip, degrees"),
    ("bone.shin_l.rot", "bone.lk", Bone::Lk, "screen-left knee, degrees"),
    ("bone.thigh_r.rot", "bone.rh", Bone::Rh, "screen-right hip, degrees"),
    ("bone.shin_r.rot", "bone.rk", Bone::Rk, "screen-right knee, degrees"),
    ("bone.eyes.x", "bone.look", Bone::Look, "eye direction, -1 (left) to 1 (right)"),
    ("bone.hand_r.thumb", "bone.thumb", Bone::Thumb, "right-hand thumbs-up, 0 to 1"),
    ("bone.hips.drop", "bone.drop", Bone::Drop, "lower the hips, px; the legs bend (crouch, sit)"),
    ("bone.thigh.fold", "bone.fold", Bone::Fold, "foreshorten the thighs 0 to 0.9 (sitting, thighs toward the camera)"),
    ("bone.shin.fold", "bone.kneel", Bone::Kneel, "foreshorten the shins 0 to 0.9 (kneeling, shins pointing back)"),
    ("bone.root.tilt", "bone.tilt", Bone::Tilt, "rotate the whole body around the hips, degrees (lying down)"),
    ("bone.root.ground", "bone.ground", Bone::Ground, "0 to 1: rest the lowest point of the body on the ground"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookField {
    Skin,
    Hair,
    Top,
    Bottom,
    Shoes,
    Accent,
    HairStyle,
    Build,
    Accessory,
    Style,
    Dress,
}

const LOOKS: &[(&str, &str, LookField)] = &[
    ("look.skin", "skin", LookField::Skin),
    ("look.hair", "hair", LookField::Hair),
    ("look.top", "top", LookField::Top),
    ("look.bottom", "bottom", LookField::Bottom),
    ("look.shoes", "shoes", LookField::Shoes),
    ("look.accent", "accent", LookField::Accent),
    ("look.hairStyle", "look.hair_style", LookField::HairStyle),
    ("look.build", "build", LookField::Build),
    ("look.accessory", "accessory", LookField::Accessory),
    ("look.style", "look.figure_style", LookField::Style),
    ("look.dress", "dress", LookField::Dress),
];

/// The property a track animates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    X,
    Y,
    Scale,
    ScaleX,
    ScaleY,
    Rotation,
    Opacity,
    Flip,
    Bone(Bone),
    Look(LookField),
    Outfit,
    Expression,
    CameraX,
    CameraY,
    CameraZoom,
}

/// What a track is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Any item.
    Item,
    /// Character items only.
    Character,
    /// The scene itself (camera).
    Scene,
}

/// How values on a channel behave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Number,
    Color,
    /// Bools, enums and outfits switch on the key's frame.
    Step,
}

const SIMPLE: &[(&str, &[&str], Channel)] = &[
    ("transform.x", &["x", "position.x"], Channel::X),
    ("transform.y", &["y", "position.y"], Channel::Y),
    ("transform.scale", &["scale"], Channel::Scale),
    ("transform.scaleX", &["transform.scale_x", "scaleX"], Channel::ScaleX),
    ("transform.scaleY", &["transform.scale_y", "scaleY"], Channel::ScaleY),
    ("transform.rotation", &["rotation", "transform.rot"], Channel::Rotation),
    ("transform.opacity", &["opacity"], Channel::Opacity),
    ("transform.flip", &["flip"], Channel::Flip),
    ("outfit", &["costume", "look.outfit"], Channel::Outfit),
    ("expression", &["face", "look.expression"], Channel::Expression),
    ("camera.x", &[], Channel::CameraX),
    ("camera.y", &[], Channel::CameraY),
    ("camera.zoom", &[], Channel::CameraZoom),
];

impl Channel {
    /// Parse a path (canonical or alias).
    pub fn parse(path: &str) -> Result<Channel, String> {
        let p = path.trim();
        for (canon, aliases, ch) in SIMPLE {
            if p == *canon || aliases.contains(&p) {
                return Ok(*ch);
            }
        }
        for (canon, alias, b, _) in BONES {
            // `bone.le.rot` is accepted too.
            if p == *canon || p == *alias || p.strip_suffix(".rot") == Some(alias) {
                return Ok(Channel::Bone(*b));
            }
        }
        for (canon, alias, f) in LOOKS {
            if p == *canon || p == *alias {
                return Ok(Channel::Look(*f));
            }
        }
        Err(format!("unknown track path {path:?}; valid paths: {}", all_paths().join(", ")))
    }

    /// The canonical spelling, used as the track's key in the document.
    pub fn path(self) -> &'static str {
        if let Some((c, _, _)) = SIMPLE.iter().find(|(_, _, ch)| *ch == self) {
            return c;
        }
        match self {
            Channel::Bone(b) => BONES.iter().find(|e| e.2 == b).map(|e| e.0).unwrap(),
            Channel::Look(f) => LOOKS.iter().find(|e| e.2 == f).map(|e| e.0).unwrap(),
            _ => unreachable!(),
        }
    }

    pub fn scope(self) -> Scope {
        match self {
            Channel::X | Channel::Y | Channel::Scale | Channel::ScaleX | Channel::ScaleY | Channel::Rotation | Channel::Opacity | Channel::Flip => Scope::Item,
            Channel::Bone(_) | Channel::Look(_) | Channel::Outfit | Channel::Expression => Scope::Character,
            Channel::CameraX | Channel::CameraY | Channel::CameraZoom => Scope::Scene,
        }
    }

    pub fn kind(self) -> ValueKind {
        match self {
            Channel::Flip | Channel::Outfit | Channel::Expression => ValueKind::Step,
            Channel::Look(f) => match f {
                LookField::Skin | LookField::Hair | LookField::Top | LookField::Bottom | LookField::Shoes | LookField::Accent => ValueKind::Color,
                _ => ValueKind::Step,
            },
            _ => ValueKind::Number,
        }
    }

    /// Check a key value for this channel and return it in canonical form.
    /// `character` is the item the track belongs to (for outfit names).
    pub fn normalize(self, v: &Value, character: Option<&CharacterItem>) -> Result<Value, String> {
        let path = self.path();
        let bad = |want: &str| format!("{path}: expected {want}, got {v}");
        match self.kind() {
            ValueKind::Number => {
                let n = v.as_f64().ok_or_else(|| bad("a number"))?;
                if !n.is_finite() {
                    return Err(bad("a finite number"));
                }
                Ok(Value::from(n))
            }
            ValueKind::Color => {
                let c = v.as_str().and_then(Color::parse).ok_or_else(|| bad("a colour like \"#ffcc99\""))?;
                Ok(Value::from(c.to_hex()))
            }
            ValueKind::Step => match self {
                Channel::Flip | Channel::Look(LookField::Dress) => v.as_bool().map(Value::from).ok_or_else(|| bad("true or false")),
                Channel::Expression => canon::<Expression>(v).map_err(|_| bad("an expression (neutral, happy, sad, angry, surprised, thinking, wink, worried, confused)")),
                Channel::Look(LookField::HairStyle) => canon::<HairStyle>(v).map_err(|_| bad("a hair style (short, spiky, long, bun, curly, bald)")),
                Channel::Look(LookField::Build) => canon::<Build>(v).map_err(|_| bad("a build (slim, regular, broad)")),
                Channel::Look(LookField::Accessory) => canon::<Accessory>(v).map_err(|_| bad("an accessory (none, glasses, tie, cap, bow)")),
                Channel::Look(LookField::Style) => canon::<FigureStyle>(v).map_err(|_| bad("a figure style (cartoon, stick)")),
                Channel::Outfit => match v {
                    Value::String(name) => {
                        let known = name == "default" || character.is_some_and(|c| c.outfits.contains_key(name));
                        if known {
                            Ok(v.clone())
                        } else {
                            let names: Vec<&str> = character.map(|c| c.outfits.keys().map(String::as_str).collect()).unwrap_or_default();
                            Err(format!("outfit {name:?} is not defined on this character (defined: default{}{}); define it with defineOutfit or key a look object", if names.is_empty() { "" } else { ", " }, names.join(", ")))
                        }
                    }
                    Value::Object(_) => {
                        let patch: LookPatch = serde_json::from_value(v.clone()).map_err(|e| format!("outfit: {e}"))?;
                        Ok(serde_json::to_value(patch).unwrap())
                    }
                    _ => Err(bad("an outfit name or a look object like {\"top\":\"#cc3344\",\"accessory\":\"tie\"}")),
                },
                _ => unreachable!(),
            },
        }
    }

    /// Error unless this channel can live on `kind` (`None` = the scene).
    pub fn check_target(self, kind: Option<&ItemKind>) -> Result<(), String> {
        let ok = matches!(
            (self.scope(), kind),
            (Scope::Scene, None) | (Scope::Item, Some(_)) | (Scope::Character, Some(ItemKind::Character(_)))
        );
        if ok {
            return Ok(());
        }
        Err(match self.scope() {
            Scope::Scene => format!("{} is a scene track: leave out the item", self.path()),
            Scope::Character => format!("{} only applies to characters", self.path()),
            Scope::Item => format!("{} needs an item", self.path()),
        })
    }
}

fn canon<T: DeserializeOwned + Serialize>(v: &Value) -> Result<Value, serde_json::Error> {
    let t: T = serde_json::from_value(v.clone())?;
    serde_json::to_value(t)
}

/// Every canonical path, for help text and errors.
pub fn all_paths() -> Vec<&'static str> {
    let mut v: Vec<&str> = SIMPLE.iter().map(|e| e.0).collect();
    v.extend(BONES.iter().map(|e| e.0));
    v.extend(LOOKS.iter().map(|e| e.0));
    v
}
