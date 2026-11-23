//! The unified keyframe engine.
//!
//! Everything animatable is a track of keys addressed by a path
//! (`transform.x`, `bone.forearm_l.rot`, `look.skin`, `outfit`, `camera.zoom`,
//! ...). Items and scenes carry a `tracks` map; the evaluator samples it per
//! integer frame, so animation stays deterministic.
//!
//! Keyed values override the item's static fields (and its legacy `motion`),
//! so an item with no tracks renders exactly as before.

pub mod autokey;
pub mod path;
pub mod track;

pub use autokey::{autokey_item, posed_item, AUTO_CHANNELS};
pub use path::{all_paths, Bone, Channel, LookField, Scope, ValueKind};
pub use track::{Interp, Key, Track};

use crate::model::{CamKey, CharacterItem, Expression, Item, Look, LookPatch, Scene};
use crate::rig::Pose;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Tracks keyed by canonical path. A `BTreeMap` keeps files diff-friendly.
pub type Tracks = BTreeMap<String, Track>;

/// Sort keys, drop duplicates and empty tracks, and move alias paths to their
/// canonical spelling (for hand-written files).
pub fn normalize_tracks(tracks: &mut Tracks) {
    let old = std::mem::take(tracks);
    for (p, mut t) in old {
        let key = Channel::parse(&p).map(|c| c.path().to_string()).unwrap_or(p);
        match tracks.get_mut(&key) {
            Some(existing) => existing.upsert(t.keys),
            None => {
                t.normalize();
                tracks.insert(key, t);
            }
        }
    }
    tracks.retain(|_, t| !t.keys.is_empty());
}

/// Iterate tracks whose path parses (unknown paths in hand-written files are ignored).
fn channels(tracks: &Tracks) -> impl Iterator<Item = (Channel, &Track)> {
    tracks.iter().filter_map(|(p, t)| Channel::parse(p).ok().map(|c| (c, t)))
}

fn number(tracks: &Tracks, ch: Channel, frame: f32) -> Option<f32> {
    tracks.get(ch.path()).and_then(|t| t.number(frame))
}

/// An item's keyed transform at a frame, before enter/exit effects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemTransform {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// Extra per-axis multipliers (squash and stretch), 1 = none.
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub flip: bool,
    /// Moving this frame (drives auto-walk), and which way (-1, 0, 1).
    pub moving: bool,
    pub dir: f32,
}

/// Position from the legacy one-segment `motion`, ignoring tracks.
fn motion_position(item: &Item, frame: f32) -> (f32, f32) {
    match item.motion {
        Some(m) if m.end > m.start => {
            let e = m.ease.apply((frame - m.start as f32) / (m.end - m.start) as f32);
            (item.x + (m.x - item.x) * e, item.y + (m.y - item.y) * e)
        }
        _ => (item.x, item.y),
    }
}

/// Position at a (possibly fractional) frame: tracks win over `motion`.
pub fn item_xy(item: &Item, frame: f32) -> (f32, f32) {
    let (mx, my) = motion_position(item, frame);
    (number(&item.tracks, Channel::X, frame).unwrap_or(mx), number(&item.tracks, Channel::Y, frame).unwrap_or(my))
}

pub fn item_transform(item: &Item, frame: u32) -> ItemTransform {
    let f = frame as f32;
    let (x, y) = item_xy(item, f);
    let t = &item.tracks;
    let flip = t.get(Channel::Flip.path()).and_then(|tr| tr.step(f)).and_then(|v| v.as_bool()).unwrap_or(item.flip);

    // Moving: the legacy motion's own window, or keyed x/y changing this frame.
    let keyed = t.contains_key(Channel::X.path()) || t.contains_key(Channel::Y.path());
    let (moving, dir) = if keyed {
        let (nx, ny) = item_xy(item, f + 1.0);
        let (dx, dy) = (nx - x, ny - y);
        let moving = dx.abs() > 0.05 || dy.abs() > 0.05;
        (moving, if dx.abs() > 0.05 { dx.signum() } else { 0.0 })
    } else {
        match item.motion {
            Some(m) if m.end > m.start => (frame >= m.start && frame < m.end, (m.x - item.x).signum()),
            _ => (false, 0.0),
        }
    };

    ItemTransform {
        x,
        y,
        scale: number(t, Channel::Scale, f).unwrap_or(item.scale),
        scale_x: number(t, Channel::ScaleX, f).unwrap_or(1.0),
        scale_y: number(t, Channel::ScaleY, f).unwrap_or(1.0),
        rotation: number(t, Channel::Rotation, f).unwrap_or(item.rotation),
        opacity: number(t, Channel::Opacity, f).unwrap_or(item.opacity),
        flip,
        moving,
        dir,
    }
}

/// Keyed bones override the action's pose.
pub fn apply_bones(tracks: &Tracks, pose: &mut Pose, frame: u32) {
    for (ch, tr) in channels(tracks) {
        let Channel::Bone(b) = ch else { continue };
        let Some(v) = tr.number(frame as f32) else { continue };
        let slot = match b {
            Bone::Rise => &mut pose.rise,
            Bone::Bob => &mut pose.bob,
            Bone::Lean => &mut pose.lean,
            Bone::Head => &mut pose.head,
            Bone::Ls => &mut pose.ls,
            Bone::Le => &mut pose.le,
            Bone::Rs => &mut pose.rs,
            Bone::Re => &mut pose.re,
            Bone::Lh => &mut pose.lh,
            Bone::Lk => &mut pose.lk,
            Bone::Rh => &mut pose.rh,
            Bone::Rk => &mut pose.rk,
            Bone::Look => &mut pose.look,
            Bone::Thumb => &mut pose.thumb,
        };
        *slot = v;
    }
}

/// Whether any track changes the look (lets the evaluator skip a clone).
fn touches_look(tracks: &Tracks) -> bool {
    channels(tracks).any(|(c, _)| matches!(c, Channel::Look(_) | Channel::Outfit))
}

/// The character's look at a frame: base look, then the keyed outfit, then
/// single keyed look properties (the most specific key wins).
pub fn resolve_look<'a>(c: &'a CharacterItem, tracks: &Tracks, frame: u32) -> Cow<'a, Look> {
    if !touches_look(tracks) {
        return Cow::Borrowed(&c.look);
    }
    let f = frame as f32;
    let mut look = c.look.clone();
    if let Some(v) = tracks.get(Channel::Outfit.path()).and_then(|t| t.step(f)) {
        let patch = match v {
            serde_json::Value::String(name) => c.outfits.get(name).cloned(),
            obj @ serde_json::Value::Object(_) => serde_json::from_value::<LookPatch>(obj.clone()).ok(),
            _ => None,
        };
        if let Some(p) = patch {
            p.apply(&mut look);
        }
    }
    for (ch, tr) in channels(tracks) {
        let Channel::Look(field) = ch else { continue };
        let slot = match field {
            LookField::Skin => Some(&mut look.skin),
            LookField::Hair => Some(&mut look.hair),
            LookField::Top => Some(&mut look.top),
            LookField::Bottom => Some(&mut look.bottom),
            LookField::Shoes => Some(&mut look.shoes),
            LookField::Accent => Some(&mut look.accent),
            _ => None,
        };
        if let Some(slot) = slot {
            if let Some(col) = tr.color(f) {
                *slot = col;
            }
            continue;
        }
        let Some(v) = tr.step(f).cloned() else { continue };
        match field {
            LookField::HairStyle => look.hair_style = serde_json::from_value(v).unwrap_or(look.hair_style),
            LookField::Build => look.build = serde_json::from_value(v).unwrap_or(look.build),
            LookField::Accessory => look.accessory = serde_json::from_value(v).unwrap_or(look.accessory),
            LookField::Style => look.style = serde_json::from_value(v).unwrap_or(look.style),
            LookField::Dress => look.dress = v.as_bool().unwrap_or(look.dress),
            _ => {}
        }
    }
    Cow::Owned(look)
}

/// The keyed expression, or the item's own.
pub fn expression(c: &CharacterItem, tracks: &Tracks, frame: u32) -> Expression {
    tracks
        .get(Channel::Expression.path())
        .and_then(|t| t.step(frame as f32))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(c.expression)
}

/// Camera tracks override the scene's from/to move channel by channel.
pub fn camera(scene: &Scene, base: CamKey, frame: u32) -> CamKey {
    let t = &scene.tracks;
    if t.is_empty() {
        return base;
    }
    let f = frame as f32;
    CamKey {
        x: number(t, Channel::CameraX, f).unwrap_or(base.x),
        y: number(t, Channel::CameraY, f).unwrap_or(base.y),
        zoom: number(t, Channel::CameraZoom, f).unwrap_or(base.zoom),
    }
}
