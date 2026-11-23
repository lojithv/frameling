//! Auto-keying: editing a property at the playhead records a keyframe.
//!
//! The editor shows an item *posed* at the playhead (keyed values baked into
//! its plain fields, see [`posed_item`]). When the user changes that copy and
//! sends it back with [`autokey_item`], every animatable property that
//! changed is written as follows:
//!
//! - the property already has a track: a key is added (or replaced) at the
//!   playhead;
//! - it has no track yet and the playhead is past the item's first frame
//!   (and auto-key is on): a track is created with the old value keyed at
//!   the item's first frame and the new value at the playhead, so "set it up,
//!   move ahead, change it" animates from the first setting to the new one;
//! - otherwise (at the item's first frame, or auto-key off) the plain field
//!   changes, as before keyframes existed.
//!
//! Everything that is not animatable (name, action, timing, voice, ...) is
//! copied from the edited item unchanged.

use super::path::{Channel, LookField};
use super::track::Key;
use super::{expression, item_transform, item_xy, resolve_look};
use crate::geom::Color;
use crate::model::{Item, ItemKind};
use serde_json::Value;

/// The properties the editor can change directly and that auto-keying covers.
pub const AUTO_CHANNELS: &[Channel] = &[
    Channel::X,
    Channel::Y,
    Channel::Scale,
    Channel::Rotation,
    Channel::Opacity,
    Channel::Flip,
    Channel::Expression,
    Channel::View,
    Channel::Look(LookField::Skin),
    Channel::Look(LookField::Hair),
    Channel::Look(LookField::Top),
    Channel::Look(LookField::Bottom),
    Channel::Look(LookField::Shoes),
    Channel::Look(LookField::Accent),
    Channel::Look(LookField::HairStyle),
    Channel::Look(LookField::Build),
    Channel::Look(LookField::Accessory),
    Channel::Look(LookField::Style),
    Channel::Look(LookField::Dress),
];

/// An f32 field as JSON without float noise (0.95, not 0.949999988).
fn num(x: f32) -> Value {
    Value::from(x.to_string().parse::<f64>().unwrap_or(x as f64))
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// An item's plain (unkeyed) field for a channel, in track-value form.
/// `None` when the channel does not apply to this item.
pub fn field(item: &Item, ch: Channel) -> Option<Value> {
    let c = match &item.kind {
        ItemKind::Character(c) => Some(c),
        _ => None,
    };
    Some(match ch {
        Channel::X => num(item.x),
        Channel::Y => num(item.y),
        Channel::Scale => num(item.scale),
        Channel::Rotation => num(item.rotation),
        Channel::Opacity => num(item.opacity),
        Channel::Flip => Value::from(item.flip),
        Channel::Expression => to_value(&c?.expression),
        Channel::View => to_value(&c?.view),
        Channel::Look(f) => {
            let l = &c?.look;
            match f {
                LookField::Skin => Value::from(l.skin.to_hex()),
                LookField::Hair => Value::from(l.hair.to_hex()),
                LookField::Top => Value::from(l.top.to_hex()),
                LookField::Bottom => Value::from(l.bottom.to_hex()),
                LookField::Shoes => Value::from(l.shoes.to_hex()),
                LookField::Accent => Value::from(l.accent.to_hex()),
                LookField::HairStyle => to_value(&l.hair_style),
                LookField::Build => to_value(&l.build),
                LookField::Accessory => to_value(&l.accessory),
                LookField::Style => to_value(&l.style),
                LookField::Dress => Value::from(l.dress),
            }
        }
        _ => return None,
    })
}

/// Write a track-value into the item's plain field (ignored if it does not fit).
pub fn set_field(item: &mut Item, ch: Channel, v: &Value) {
    let num = v.as_f64().map(|n| n as f32);
    let col = || v.as_str().and_then(Color::parse);
    match ch {
        Channel::X => item.x = num.unwrap_or(item.x),
        Channel::Y => item.y = num.unwrap_or(item.y),
        Channel::Scale => item.scale = num.unwrap_or(item.scale),
        Channel::Rotation => item.rotation = num.unwrap_or(item.rotation),
        Channel::Opacity => item.opacity = num.unwrap_or(item.opacity),
        Channel::Flip => item.flip = v.as_bool().unwrap_or(item.flip),
        _ => {
            let ItemKind::Character(c) = &mut item.kind else { return };
            let l = &mut c.look;
            fn parse<T: serde::de::DeserializeOwned>(cur: &mut T, v: &Value) {
                if let Ok(x) = serde_json::from_value(v.clone()) {
                    *cur = x;
                }
            }
            match ch {
                Channel::Expression => parse(&mut c.expression, v),
                Channel::View => parse(&mut c.view, v),
                Channel::Look(LookField::Skin) => l.skin = col().unwrap_or(l.skin),
                Channel::Look(LookField::Hair) => l.hair = col().unwrap_or(l.hair),
                Channel::Look(LookField::Top) => l.top = col().unwrap_or(l.top),
                Channel::Look(LookField::Bottom) => l.bottom = col().unwrap_or(l.bottom),
                Channel::Look(LookField::Shoes) => l.shoes = col().unwrap_or(l.shoes),
                Channel::Look(LookField::Accent) => l.accent = col().unwrap_or(l.accent),
                Channel::Look(LookField::HairStyle) => parse(&mut l.hair_style, v),
                Channel::Look(LookField::Build) => parse(&mut l.build, v),
                Channel::Look(LookField::Accessory) => parse(&mut l.accessory, v),
                Channel::Look(LookField::Style) => parse(&mut l.style, v),
                Channel::Look(LookField::Dress) => l.dress = v.as_bool().unwrap_or(l.dress),
                _ => {}
            }
        }
    }
}

/// The item as it looks at a scene-local frame: keyed values (and the legacy
/// motion path) baked into its plain fields. Tracks are kept.
pub fn posed_item(item: &Item, frame: u32) -> Item {
    let mut out = item.clone();
    let (x, y) = item_xy(item, frame as f32);
    let t = item_transform(item, frame);
    out.x = x;
    out.y = y;
    out.scale = t.scale;
    out.rotation = t.rotation;
    out.opacity = t.opacity;
    out.flip = t.flip;
    if let (ItemKind::Character(src), ItemKind::Character(dst)) = (&item.kind, &mut out.kind) {
        dst.expression = expression(src, &item.tracks, frame);
        dst.view = super::view(src, &item.tracks, frame);
        // Only the look fields with their own tracks: an outfit switch stays a
        // switch and is not baked into the base look.
        let look = resolve_look(src, &item.tracks, frame);
        let mut posed = item.clone();
        if let ItemKind::Character(pc) = &mut posed.kind {
            pc.look = look.into_owned();
        }
        for ch in AUTO_CHANNELS.iter().copied().filter(|c| matches!(c, Channel::Look(_))) {
            if item.tracks.contains_key(ch.path()) {
                if let Some(v) = field(&posed, ch) {
                    set_field(&mut out, ch, &v);
                }
            }
        }
    }
    out
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-4,
        _ => match (a.as_str().and_then(Color::parse), b.as_str().and_then(Color::parse)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}

/// Merge an edited, posed copy of `current` back into the document.
///
/// `edited` is what the user sees and changed at `frame` (a scene-local
/// frame). Returns the item to store. With `auto` off, unkeyed properties
/// change their plain field instead of starting a track; properties that
/// already have a track are always keyed, or the edit would not show.
pub fn autokey_item(current: &Item, edited: &Item, frame: u32, auto: bool) -> Result<Item, String> {
    if current.id != edited.id {
        return Err(format!("autoKey: item id changed from {:?} to {:?}", current.id, edited.id));
    }
    let before = posed_item(current, frame);
    let first = current.start;
    // Non-animatable fields come from the edit; animatable ones start from the
    // document and only change below.
    let mut out = edited.clone();
    out.tracks = current.tracks.clone();
    for &ch in AUTO_CHANNELS {
        if let Some(v) = field(current, ch) {
            set_field(&mut out, ch, &v);
        }
    }
    for &ch in AUTO_CHANNELS {
        let (Some(old), Some(new)) = (field(&before, ch), field(edited, ch)) else { continue };
        if same(&old, &new) {
            continue;
        }
        let character = match &out.kind {
            ItemKind::Character(c) => Some(c),
            _ => None,
        };
        let new = ch.normalize(&new, character)?;
        if let Some(track) = out.tracks.get_mut(ch.path()) {
            // Keep the shape of a key the user already set up.
            let interp = track.keys.iter().find(|k| k.frame == frame).map(|k| k.interp).unwrap_or_default();
            track.upsert([Key { frame, value: new, interp }]);
        } else if auto && frame > first {
            let start = field(&posed_item(current, first), ch).unwrap_or(old);
            let start = ch.normalize(&start, character)?;
            let track = out.tracks.entry(ch.path().to_string()).or_default();
            track.upsert([Key::new(first, start), Key::new(frame, new)]);
        } else {
            set_field(&mut out, ch, &new);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Editor};
    use crate::model::{CharacterItem, Expression, Project, Scene};

    fn project() -> Project {
        let mut p = Project::default();
        let ch = Item { id: "maya".into(), kind: ItemKind::Character(CharacterItem::default()), ..Default::default() };
        p.scenes = vec![Scene { id: "s".into(), duration: 300, items: vec![ch], ..Default::default() }];
        p
    }

    fn edit(ed: &mut Editor, frame: u32, f: impl FnOnce(&mut Item)) {
        let cur = ed.project.scenes[0].items[0].clone();
        let mut it = posed_item(&cur, frame);
        f(&mut it);
        ed.apply(&Command::AutoKey { scene: "s".into(), item: it, frame, auto: true }, None).unwrap();
    }

    fn item(ed: &Editor) -> &Item {
        &ed.project.scenes[0].items[0]
    }

    fn character(it: &Item) -> &CharacterItem {
        match &it.kind {
            ItemKind::Character(c) => c,
            _ => panic!(),
        }
    }

    #[test]
    fn first_frame_edits_stay_plain() {
        let mut ed = Editor::new(project());
        edit(&mut ed, 0, |it| it.x = 200.0);
        assert_eq!(item(&ed).x, 200.0);
        assert!(item(&ed).tracks.is_empty());
    }

    #[test]
    fn set_up_move_ahead_change_makes_keys() {
        let mut ed = Editor::new(project());
        edit(&mut ed, 0, |it| it.x = 200.0);
        // Two seconds later: move, rotate, grow, recolour, change the face.
        edit(&mut ed, 60, |it| {
            it.x = 800.0;
            it.rotation = 30.0;
            it.scale = 2.0;
            if let ItemKind::Character(c) = &mut it.kind {
                c.look.skin = Color::parse("#335577").unwrap();
                c.expression = Expression::Surprised;
            }
        });
        let it = item(&ed);
        // The plain fields keep the first setting.
        assert_eq!(it.x, 200.0);
        assert_eq!(it.rotation, 0.0);
        for (path, n) in [("transform.x", 2), ("transform.rotation", 2), ("transform.scale", 2), ("look.skin", 2), ("expression", 2)] {
            assert_eq!(it.tracks[path].keys.len(), n, "{path}");
        }
        assert!(!it.tracks.contains_key("transform.y"), "unchanged properties get no track");
        assert_eq!(it.tracks["transform.x"].keys[0].value, Value::from(200.0));
        // Halfway, it's in between; at and after the key it holds the new value.
        let mid = posed_item(it, 30);
        assert_eq!(mid.x, 500.0);
        assert_eq!(mid.rotation, 15.0);
        assert_eq!(character(&mid).expression, Expression::Neutral);
        let end = posed_item(it, 90);
        assert_eq!(end.x, 800.0);
        assert_eq!(character(&end).expression, Expression::Surprised);
        assert_eq!(character(&end).look.skin, Color::parse("#335577").unwrap());
        assert_ne!(character(&mid).look.skin, character(&end).look.skin, "colours blend");

        // A third change adds a third key; editing at the first frame now keys too.
        edit(&mut ed, 120, |it| it.x = 400.0);
        edit(&mut ed, 0, |it| it.x = 100.0);
        let xs: Vec<_> = item(&ed).tracks["transform.x"].keys.iter().map(|k| (k.frame, k.value.as_f64().unwrap())).collect();
        assert_eq!(xs, vec![(0, 100.0), (60, 800.0), (120, 400.0)]);

        // Undo takes back one edit at a time.
        ed.undo();
        assert_eq!(item(&ed).tracks["transform.x"].keys[0].value, Value::from(200.0));
    }

    #[test]
    fn drag_updates_one_key() {
        let mut ed = Editor::new(project());
        for x in [650.0, 700.0, 750.0] {
            let cur = item(&ed).clone();
            let mut it = posed_item(&cur, 45);
            it.x = x;
            ed.apply(&Command::AutoKey { scene: "s".into(), item: it, frame: 45, auto: true }, Some("drag")).unwrap();
        }
        let keys = &item(&ed).tracks["transform.x"].keys;
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[1].value, Value::from(750.0));
        ed.undo();
        assert!(item(&ed).tracks.is_empty(), "one drag is one undo step");
    }

    #[test]
    fn auto_off_changes_plain_fields_but_keys_animated_ones() {
        let mut ed = Editor::new(project());
        edit(&mut ed, 60, |it| it.y = 500.0);
        let cur = item(&ed).clone();
        let mut it = posed_item(&cur, 30);
        it.x = 10.0;
        it.y = 400.0;
        it.name = "Maya".into();
        ed.apply(&Command::AutoKey { scene: "s".into(), item: it, frame: 30, auto: false }, None).unwrap();
        let it = item(&ed);
        assert_eq!(it.x, 10.0);
        assert!(!it.tracks.contains_key("transform.x"));
        assert_eq!(it.tracks["transform.y"].keys.len(), 3);
        assert_eq!(it.name, "Maya");
    }

    #[test]
    fn items_that_start_later_key_from_their_first_frame() {
        let mut p = project();
        p.scenes[0].items[0].start = 30;
        let mut ed = Editor::new(p);
        edit(&mut ed, 30, |it| it.opacity = 0.5);
        assert!(item(&ed).tracks.is_empty());
        edit(&mut ed, 90, |it| it.opacity = 1.0);
        let frames: Vec<u32> = item(&ed).tracks["transform.opacity"].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![30, 90]);
    }
}
