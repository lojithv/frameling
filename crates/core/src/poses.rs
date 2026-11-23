//! The pose library in use: poses held over the animation (strips), partial
//! poses (masks), mirroring, poses written as keys, and animations chained
//! from poses.
//!
//! A pose is just joint values on the shared skeleton, so any pose fits any
//! character: cartoon, stick figure or one made in the character creator.

use crate::anim::path::BONES;
use crate::model::{Id, Project};
use crate::pack::{ActionDef, ActionKey, Library, PoseDef};
use crate::rig::Pose;
use serde::{Deserialize, Serialize};

/// Which joints a pose drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PoseMask {
    #[default]
    Full,
    /// Head, spine and arms: plays over a walk.
    Upper,
    /// Hips and legs.
    Lower,
    /// The screen-left arm.
    ArmL,
    /// The screen-right arm (and thumbs-up).
    ArmR,
    /// Head tilt and eyes.
    Head,
}

/// Pose fields by name, in the order the mask table uses.
type Field = fn(&mut Pose) -> &mut f32;
const UPPER: &[Field] = &[|p| &mut p.bob, |p| &mut p.lean, |p| &mut p.head, |p| &mut p.ls, |p| &mut p.le, |p| &mut p.rs, |p| &mut p.re, |p| &mut p.look, |p| &mut p.thumb];
const LOWER: &[Field] = &[|p| &mut p.rise, |p| &mut p.lh, |p| &mut p.lk, |p| &mut p.rh, |p| &mut p.rk, |p| &mut p.drop, |p| &mut p.fold, |p| &mut p.kneel, |p| &mut p.tilt, |p| &mut p.ground];
const ARM_L: &[Field] = &[|p| &mut p.ls, |p| &mut p.le];
const ARM_R: &[Field] = &[|p| &mut p.rs, |p| &mut p.re, |p| &mut p.thumb];
const HEAD: &[Field] = &[|p| &mut p.head, |p| &mut p.look];

impl PoseMask {
    fn fields(self) -> Vec<Field> {
        match self {
            PoseMask::Full => UPPER.iter().chain(LOWER).copied().collect(),
            PoseMask::Upper => UPPER.to_vec(),
            PoseMask::Lower => LOWER.to_vec(),
            PoseMask::ArmL => ARM_L.to_vec(),
            PoseMask::ArmR => ARM_R.to_vec(),
            PoseMask::Head => HEAD.to_vec(),
        }
    }

}

/// Swap left and right: the same pose facing the other way.
pub fn mirror(p: &Pose) -> Pose {
    Pose {
        ls: p.rs,
        le: p.re,
        rs: p.ls,
        re: p.le,
        lh: p.rh,
        lk: p.rk,
        rh: p.lh,
        rk: p.lk,
        lean: -p.lean,
        head: -p.head,
        look: -p.look,
        tilt: -p.tilt,
        ..*p
    }
}

/// Blend `target` into `base` by `w` (0..1), only on the mask's joints.
pub fn blend(base: &Pose, target: &Pose, w: f32, mask: PoseMask) -> Pose {
    let mut out = *base;
    let mut t = *target;
    let mut b = *base;
    for f in mask.fields() {
        let (from, to) = (*f(&mut b), *f(&mut t));
        *f(&mut out) = from + (to - from) * w;
    }
    out
}

/// A pose held on a character from `start` to `end` (scene frames), blending
/// in over `blend_in` frames and out over `blend_out` frames.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PoseStrip {
    /// A pose id (library or project), or give `custom` instead.
    pub pose: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<Pose>,
    pub start: u32,
    pub end: u32,
    pub blend_in: u32,
    pub blend_out: u32,
    pub mask: PoseMask,
    pub mirror: bool,
    /// Add a little breathing so a held pose never looks frozen.
    pub breathe: bool,
}

impl Default for PoseStrip {
    fn default() -> Self {
        PoseStrip { pose: String::new(), custom: None, start: 0, end: 60, blend_in: 8, blend_out: 8, mask: PoseMask::Full, mirror: false, breathe: true }
    }
}

impl PoseStrip {
    /// Blend weight at a frame: 0 outside, ramping in and out (smoothly).
    pub fn weight(&self, frame: u32) -> f32 {
        if frame < self.start || frame >= self.end.max(self.start + 1) {
            return 0.0;
        }
        let f = frame as f32;
        let mut w: f32 = 1.0;
        if self.blend_in > 0 {
            w = w.min((f - self.start as f32 + 1.0) / self.blend_in as f32);
        }
        if self.blend_out > 0 {
            w = w.min((self.end as f32 - f) / self.blend_out as f32);
        }
        let w = w.clamp(0.0, 1.0);
        w * w * (3.0 - 2.0 * w)
    }
}

/// Find a pose: the project's own first, then the library.
pub fn find<'a>(project: Option<&'a Project>, lib: &'a Library, id: &str) -> Option<&'a PoseDef> {
    project.and_then(|p| p.poses.get(id)).or_else(|| lib.poses.get(id))
}

/// The pose a strip asks for (mirrored if set), or None if it is unknown.
pub fn strip_pose(project: Option<&Project>, lib: &Library, s: &PoseStrip) -> Option<Pose> {
    let p = match &s.custom {
        Some(p) => *p,
        None => find(project, lib, &s.pose)?.pose,
    };
    Some(if s.mirror { mirror(&p) } else { p })
}

/// Apply every strip active at `frame` on top of the animated pose.
pub fn apply_strips(base: Pose, strips: &[PoseStrip], frame: u32, fps: f32, project: &Project, lib: &Library) -> Pose {
    let mut out = base;
    for s in strips {
        let w = s.weight(frame);
        if w <= 0.0 {
            continue;
        }
        let Some(mut target) = strip_pose(Some(project), lib, s) else { continue };
        if s.breathe {
            let t = (frame - s.start) as f32 / fps;
            target.bob += 1.4 * (t * std::f32::consts::TAU / 3.4).sin() + 0.6;
            target.head += 0.8 * (t * std::f32::consts::TAU / 4.1).sin();
            target.ls += 1.2 * (t * std::f32::consts::TAU / 3.4).sin();
            target.rs += 1.2 * (t * std::f32::consts::TAU / 3.4).sin();
        }
        out = blend(&out, &target, w, s.mask);
    }
    out
}

/// The bone track values that hold a pose (for writing it as keys).
pub fn pose_keys(p: &Pose, mask: PoseMask) -> Vec<(&'static str, f32)> {
    let mut full = *p;
    let mut out = vec![];
    for (path, _, bone, _) in BONES {
        // Read the channel by writing a marker through apply_bones' mapping.
        let mut probe = Pose::default();
        let track: crate::anim::Tracks = std::iter::once((path.to_string(), crate::anim::Track { keys: vec![crate::anim::Key::new(0, 12345.0)] })).collect();
        crate::anim::apply_bones(&track, &mut probe, 0);
        let _ = bone;
        let fields = mask.fields();
        for f in &fields {
            if *f(&mut probe) == 12345.0 {
                out.push((*path, *f(&mut full)));
            }
        }
    }
    out
}

/// One step of an animation chained from poses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChainStep {
    pub pose: Id,
    /// Seconds to hold this pose before moving on.
    pub hold: f32,
    pub mirror: bool,
}

impl Default for ChainStep {
    fn default() -> Self {
        ChainStep { pose: String::new(), hold: 0.0, mirror: false }
    }
}

/// Chain poses into an animation: hold each for its `hold`, then take `gap`
/// seconds to move to the next. A looping animation also moves back to the first.
pub fn chain(id: &str, name: &str, steps: &[ChainStep], gap: f32, looped: bool, project: Option<&Project>, lib: &Library) -> Result<ActionDef, String> {
    if steps.is_empty() {
        return Err("give at least one pose".into());
    }
    let gap = gap.max(0.05);
    let mut keys_s: Vec<(f32, Pose)> = vec![];
    let mut t = 0.0;
    for (i, s) in steps.iter().enumerate() {
        let def = find(project, lib, &s.pose).ok_or_else(|| format!("unknown pose {:?}; list_poses shows the ids", s.pose))?;
        let p = if s.mirror { mirror(&def.pose) } else { def.pose };
        keys_s.push((t, p));
        if s.hold > 0.0 {
            t += s.hold;
            keys_s.push((t, p));
        }
        if i + 1 < steps.len() || looped {
            t += gap;
        }
    }
    let duration = if t <= 0.0 { 1.0 } else { t };
    let keys = keys_s.into_iter().map(|(at, pose)| ActionKey { t: (at / duration).min(1.0), pose }).collect();
    Ok(ActionDef { id: id.into(), name: name.into(), duration, r#loop: looped, keys })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CharacterItem, Item, ItemKind, Scene};

    #[test]
    fn library_has_a_starter_set_in_every_group() {
        let lib = Library::builtin();
        assert!(lib.poses.len() >= 30, "{} poses", lib.poses.len());
        for cat in ["Standing", "Gestures", "Sitting and kneeling", "Lying", "Motion keys"] {
            assert!(lib.poses.values().any(|p| p.category == cat), "no {cat} poses");
        }
    }

    #[test]
    fn strips_blend_in_hold_and_out() {
        let s = PoseStrip { pose: "cheer".into(), start: 10, end: 40, blend_in: 10, blend_out: 10, ..Default::default() };
        assert_eq!(s.weight(5), 0.0);
        assert!(s.weight(12) > 0.0 && s.weight(12) < 0.5);
        assert_eq!(s.weight(25), 1.0);
        assert!(s.weight(38) < 0.5);
        assert_eq!(s.weight(40), 0.0);
    }

    #[test]
    fn upper_body_mask_leaves_the_legs_walking() {
        let walk = crate::rig::locomotion(0.2, false, 1.0);
        let point = Library::builtin().poses["point"].pose;
        let b = blend(&walk, &point, 1.0, PoseMask::Upper);
        assert_eq!((b.lh, b.rk, b.drop), (walk.lh, walk.rk, walk.drop));
        assert_eq!(b.rs, point.rs);
        let keys = pose_keys(&point, PoseMask::ArmR);
        assert!(keys.iter().any(|(p, _)| *p == "bone.upper_arm_r.rot") && keys.iter().all(|(p, _)| !p.contains("_l")));
    }

    #[test]
    fn mirror_swaps_sides_and_chains_make_animations() {
        let lib = Library::builtin();
        let p = lib.poses["point"].pose;
        let m = mirror(&p);
        assert_eq!((m.ls, m.rs), (p.rs, p.ls));
        let a = chain("wave-hello", "Wave hello", &[ChainStep { pose: "stand".into(), hold: 0.5, ..Default::default() }, ChainStep { pose: "wave".into(), hold: 1.0, ..Default::default() }], 0.4, true, None, lib).unwrap();
        assert!((a.duration - 2.3).abs() < 1e-4);
        assert_eq!(a.keys.len(), 4);
        assert!(chain("x", "x", &[ChainStep { pose: "nope".into(), ..Default::default() }], 0.4, false, None, lib).is_err());
    }

    #[test]
    fn sitting_and_lying_rest_on_the_ground_for_cartoon_and_stick() {
        // Render each grounded pose and check the lowest drawn point sits near the ground.
        let lib = Library::builtin();
        for stick in [false, true] {
            for id in ["sit-floor", "kneel", "lie-back", "crouch", "sit-chair"] {
                let mut look = if stick { crate::model::Look::stick(false) } else { crate::model::Look::default() };
                look.hair_style = crate::model::HairStyle::Bald;
                let c = CharacterItem { look, poses: vec![PoseStrip { pose: id.into(), start: 0, end: 100, blend_in: 0, blend_out: 0, breathe: false, ..Default::default() }], ..Default::default() };
                let it = Item { id: "c".into(), x: 0.0, y: 0.0, kind: ItemKind::Character(c), ..Default::default() };
                let mut sc = Scene::new("s", "s");
                sc.items.push(it);
                let p = Project { scenes: vec![sc], ..Project::default() };
                let (_, hits) = crate::eval::evaluate_scene(&p, lib, 0, 5);
                // The camera centres the 1280x720 stage, so scene y 0 is screen y 0.
                let b = hits[0].bounds;
                let bottom = b.y + b.h;
                assert!(bottom.abs() < 14.0, "{id} ({}) bottom at {bottom}", if stick { "stick" } else { "cartoon" });
            }
        }
    }
}
