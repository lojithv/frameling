//! Keyframe engine: interpolation, path grammar, commands, evaluation and
//! format compatibility.

use frameling_core::anim::{self, Channel, Interp, Key, Track};
use frameling_core::*;
use serde_json::json;

fn set_keys(scene: &str, item: Option<&str>, path: &str, keys: Vec<Key>) -> Command {
    Command::SetKeys { scene: scene.into(), item: item.map(Into::into), path: path.into(), keys, replace: false }
}

fn track(keys: Vec<Key>) -> Track {
    let mut t = Track::default();
    t.upsert(keys);
    t
}

#[test]
fn interpolation_curves_hit_their_ends() {
    for i in [Interp::Linear, Interp::EaseIn, Interp::EaseOut, Interp::EaseInOut, Interp::Back, Interp::Elastic, Interp::Bounce, Interp::Bezier([0.25, 0.1, 0.25, 1.0])] {
        assert!(i.apply(0.0).abs() < 1e-4, "{i:?} at 0");
        assert!((i.apply(1.0) - 1.0).abs() < 1e-4, "{i:?} at 1");
    }
    assert_eq!(Interp::Constant.apply(0.99), 0.0);
    // A linear bezier is linear; an ease-in bezier starts slow.
    assert!((Interp::Bezier([0.0, 0.0, 1.0, 1.0]).apply(0.3) - 0.3).abs() < 1e-3);
    assert!(Interp::Bezier([0.42, 0.0, 1.0, 1.0]).apply(0.3) < 0.2);
    // Back overshoots.
    assert!((0..100).map(|i| Interp::Back.apply(i as f32 / 100.0)).any(|v| v > 1.0));
}

#[test]
fn interp_json_forms() {
    let k: Key = serde_json::from_value(json!({ "frame": 3, "value": 1, "interp": "inOut" })).unwrap();
    assert_eq!(k.interp, Interp::EaseInOut);
    let k: Key = serde_json::from_value(json!({ "frame": 3, "value": 1, "interp": "step" })).unwrap();
    assert_eq!(k.interp, Interp::Constant);
    let k: Key = serde_json::from_value(json!({ "frame": 3, "value": 1, "interp": { "bezier": [0.1, 0.2, 0.3, 0.4] } })).unwrap();
    assert_eq!(k.interp, Interp::Bezier([0.1, 0.2, 0.3, 0.4]));
    let k: Key = serde_json::from_value(json!({ "frame": 3, "value": 1 })).unwrap();
    assert_eq!(k.interp, Interp::Linear, "linear by default");
    assert_eq!(serde_json::to_value(Key::new(1, 2).with(Interp::Elastic)).unwrap(), json!({ "frame": 1, "value": 2, "interp": "elastic" }));
}

#[test]
fn sampling_holds_outside_keys_and_blends_inside() {
    let t = track(vec![Key::new(10, 100.0), Key::new(20, 200.0).with(Interp::Constant), Key::new(30, 0.0)]);
    assert_eq!(t.number(0.0), Some(100.0));
    assert_eq!(t.number(10.0), Some(100.0));
    assert_eq!(t.number(15.0), Some(150.0));
    assert_eq!(t.number(20.0), Some(200.0));
    assert_eq!(t.number(29.0), Some(200.0), "constant holds until the next key");
    assert_eq!(t.number(30.0), Some(0.0));
    assert_eq!(t.number(1000.0), Some(0.0));
    assert_eq!(Track::default().number(5.0), None);

    let c = track(vec![Key::new(0, "#000000"), Key::new(10, "#ffffff")]);
    assert_eq!(c.color(5.0), Some(Color::rgb(128, 128, 128)));
    let s = track(vec![Key::new(0, "happy"), Key::new(10, "sad")]);
    assert_eq!(s.step(9.0), Some(&json!("happy")));
    assert_eq!(s.step(10.0), Some(&json!("sad")));
}

#[test]
fn upsert_keeps_keys_sorted_and_unique() {
    let mut t = track(vec![Key::new(30, 3), Key::new(10, 1)]);
    t.upsert(vec![Key::new(20, 2), Key::new(10, 9)]);
    let frames: Vec<u32> = t.keys.iter().map(|k| k.frame).collect();
    assert_eq!(frames, vec![10, 20, 30]);
    assert_eq!(t.keys[0].value, json!(9));
}

#[test]
fn paths_parse_aliases_to_one_canonical_name() {
    assert_eq!(Channel::parse("x").unwrap().path(), "transform.x");
    assert_eq!(Channel::parse("bone.le").unwrap().path(), "bone.forearm_l.rot");
    assert_eq!(Channel::parse("bone.le.rot").unwrap().path(), "bone.forearm_l.rot");
    assert_eq!(Channel::parse("bone.forearm_l.rot").unwrap().path(), "bone.forearm_l.rot");
    assert_eq!(Channel::parse("look.hair_style").unwrap().path(), "look.hairStyle");
    assert_eq!(Channel::parse("costume").unwrap().path(), "outfit");
    assert!(Channel::parse("bone.tail.rot").unwrap_err().contains("valid paths"));
    // Every canonical path parses back to itself.
    for p in anim::all_paths() {
        assert_eq!(Channel::parse(p).unwrap().path(), p);
    }
}

#[test]
fn set_keys_validates_values_and_targets() {
    let mut p = demo::demo_project();
    let bad = [
        set_keys("scene-1", Some("maya"), "look.skin", vec![Key::new(0, "pink")]),
        set_keys("scene-1", Some("maya"), "expression", vec![Key::new(0, "grumpy")]),
        set_keys("scene-1", Some("maya"), "transform.x", vec![Key::new(0, "far")]),
        set_keys("scene-1", Some("maya"), "camera.zoom", vec![Key::new(0, 2.0)]),
        set_keys("scene-1", None, "transform.x", vec![Key::new(0, 2.0)]),
        set_keys("scene-1", Some("maya"), "outfit", vec![Key::new(0, "tux")]),
        set_keys("scene-1", Some("nobody"), "transform.x", vec![Key::new(0, 1.0)]),
    ];
    for c in &bad {
        assert!(c.apply(&mut p).is_err(), "{c:?} should fail");
    }
    // Props can have transform keys but not bones.
    let prop = p.scenes[0].items.iter().find(|i| matches!(i.kind, ItemKind::Prop(_))).unwrap().id.clone();
    assert!(set_keys("scene-1", Some(&prop), "bone.head", vec![Key::new(0, 1.0)]).apply(&mut p).is_err());
    set_keys("scene-1", Some(&prop), "rotation", vec![Key::new(0, 0.0), Key::new(30, 90.0)]).apply(&mut p).unwrap();
    // Colours and enums are stored canonically.
    set_keys("scene-1", Some("maya"), "skin", vec![Key::new(0, "#ABC")]).apply(&mut p).unwrap();
    let maya = p.scenes[0].item("maya").unwrap();
    assert_eq!(maya.tracks["look.skin"].keys[0].value, json!("#aabbcc"));
}

#[test]
fn delete_keys_and_set_interpolation() {
    let mut p = demo::demo_project();
    set_keys("scene-1", Some("maya"), "x", (0..5).map(|i| Key::new(i * 10, i as f64)).collect()).apply(&mut p).unwrap();
    Command::SetInterpolation { scene: "scene-1".into(), item: Some("maya".into()), path: "transform.x".into(), frames: Some(vec![10]), interp: Interp::Bounce }.apply(&mut p).unwrap();
    let keys = &p.scenes[0].item("maya").unwrap().tracks["transform.x"].keys;
    assert_eq!(keys[1].interp, Interp::Bounce);
    assert_eq!(keys[0].interp, Interp::Linear);

    Command::DeleteKeys { scene: "scene-1".into(), item: Some("maya".into()), path: Some("x".into()), frames: Some(vec![0]), from: Some(30), to: None }.apply(&mut p).unwrap();
    let frames: Vec<u32> = p.scenes[0].item("maya").unwrap().tracks["transform.x"].keys.iter().map(|k| k.frame).collect();
    assert_eq!(frames, vec![10, 20]);

    Command::DeleteKeys { scene: "scene-1".into(), item: Some("maya".into()), path: None, frames: None, from: None, to: None }.apply(&mut p).unwrap();
    assert!(p.scenes[0].item("maya").unwrap().tracks.is_empty(), "empty tracks are removed");
}

#[test]
fn transform_keys_move_the_item_and_drive_auto_walk() {
    let mut p = demo::demo_project();
    set_keys("scene-1", Some("leo"), "transform.x", vec![Key::new(0, 200.0), Key::new(60, 800.0)]).apply(&mut p).unwrap();
    let leo = p.scenes[0].item("leo").unwrap();
    let t = anim::item_transform(leo, 30);
    assert_eq!(t.x, 500.0);
    assert!(t.moving && t.dir > 0.0, "keyed travel counts as walking");
    let t = anim::item_transform(leo, 70);
    assert_eq!(t.x, 800.0);
    assert!(!t.moving);
    // The rendered bounds follow the key.
    let lib = Library::builtin();
    let (_, hits) = eval::evaluate_scene(&p, lib, 0, 30);
    let b = hits.iter().find(|h| h.id == "leo").unwrap().bounds;
    assert!((b.x + b.w / 2.0 - 500.0).abs() < 60.0, "leo is drawn near x=500, got {b:?}");
}

#[test]
fn bones_override_the_action_pose() {
    let lib = Library::builtin();
    let mut p = demo::demo_project();
    let before = evaluate(&p, lib, 20);
    set_keys("scene-1", Some("leo"), "bone.forearm_r.rot", vec![Key::new(0, 0.0), Key::new(40, 150.0)]).apply(&mut p).unwrap();
    assert_ne!(evaluate(&p, lib, 20), before, "the keyed elbow changes the drawing");
    assert_eq!(evaluate(&p, lib, 20), evaluate(&p, lib, 20), "still deterministic");
}

#[test]
fn skin_colour_and_outfit_switch_over_time() {
    let mut p = demo::demo_project();
    let suit = LookPatch { top: Some(Color::rgb(20, 20, 20)), accessory: Some(Accessory::Tie), ..Default::default() };
    Command::DefineOutfit { scene: "scene-1".into(), item: "leo".into(), name: "suit".into(), outfit: Some(suit) }.apply(&mut p).unwrap();
    set_keys("scene-1", Some("leo"), "outfit", vec![Key::new(0, "default"), Key::new(30, "suit")]).apply(&mut p).unwrap();
    set_keys("scene-1", Some("leo"), "look.skin", vec![Key::new(0, "#f1c29a"), Key::new(20, "#66cc66")]).apply(&mut p).unwrap();
    set_keys("scene-1", Some("leo"), "expression", vec![Key::new(0, "neutral"), Key::new(45, "surprised")]).apply(&mut p).unwrap();
    // An inline look object works as an outfit key too.
    set_keys("scene-1", Some("leo"), "outfit", vec![Key::new(60, json!({ "hairStyle": "bald", "top": "#ff0000" }))]).apply(&mut p).unwrap();

    let leo = p.scenes[0].item("leo").unwrap();
    let ItemKind::Character(c) = &leo.kind else { unreachable!() };
    let base_top = c.look.top;
    let look = |f| anim::resolve_look(c, &leo.tracks, f).into_owned();
    assert_eq!(look(10).top, base_top);
    assert_eq!(look(10).skin, Color::parse("#f1c29a").unwrap().mix(Color::parse("#66cc66").unwrap(), 0.5));
    assert_eq!(look(29).accessory, c.look.accessory);
    assert_eq!(look(30).top, Color::rgb(20, 20, 20));
    assert_eq!(look(30).accessory, Accessory::Tie);
    assert_eq!(look(30).skin, Color::parse("#66cc66").unwrap());
    assert_eq!(look(60).hair_style, HairStyle::Bald);
    assert_eq!(anim::expression(c, &leo.tracks, 44), Expression::Neutral);
    assert_eq!(anim::expression(c, &leo.tracks, 45), Expression::Surprised);

    // Unknown fields in an outfit are rejected, so typos surface.
    assert!(set_keys("scene-1", Some("leo"), "outfit", vec![Key::new(0, json!({ "shirt": "#fff" }))]).apply(&mut p).is_err());
}

#[test]
fn camera_keys_override_the_scene_move() {
    let mut p = demo::demo_project();
    set_keys("scene-1", None, "camera.zoom", vec![Key::new(0, 1.0), Key::new(60, 2.0).with(Interp::EaseOut)]).apply(&mut p).unwrap();
    let s = &p.scenes[0];
    assert_eq!(eval::camera_at(s, 30).zoom, 1.5);
    assert_eq!(eval::camera_at(s, 90).zoom, 2.0);
    assert_eq!(eval::camera_at(s, 30).x, s.camera.from.x, "unkeyed channels keep the scene camera");
}

#[test]
fn v1_projects_load_and_keyed_projects_round_trip() {
    // A version 1 file (no tracks) loads, upgrades, and saves without tracks noise.
    let mut v1: serde_json::Value = serde_json::from_str(&demo::demo_project().to_json()).unwrap();
    v1["version"] = json!(1);
    let p = Project::from_json(&v1.to_string()).unwrap();
    assert_eq!(p.version, FORMAT_VERSION);
    assert_eq!(p, demo::demo_project());
    assert!(!p.to_json().contains("\"tracks\""));

    let mut p = demo::demo_project();
    set_keys("scene-2", Some("leo2"), "transform.scaleY", vec![Key::new(0, 1.0), Key::new(5, 0.8).with(Interp::Bezier([0.3, 0.0, 0.2, 1.4]))]).apply(&mut p).unwrap();
    set_keys("scene-2", None, "camera.x", vec![Key::new(0, 640.0), Key::new(80, 800.0)]).apply(&mut p).unwrap();
    let back = Project::from_json(&p.to_json()).unwrap();
    assert_eq!(p, back);

    // Hand-written tracks with aliases and unsorted keys are tidied on load.
    let mut raw: serde_json::Value = serde_json::from_str(&demo::demo_project().to_json()).unwrap();
    raw["scenes"][0]["items"][0]["tracks"] = json!({ "x": { "keys": [{ "frame": 20, "value": 5 }, { "frame": 0, "value": 1 }] } });
    let p = Project::from_json(&raw.to_string()).unwrap();
    let t = &p.scenes[0].items[0].tracks["transform.x"];
    assert_eq!(t.keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 20]);

    // Newer files are refused rather than misread.
    raw["version"] = json!(FORMAT_VERSION + 1);
    assert!(Project::from_json(&raw.to_string()).is_err());
}

#[test]
fn key_edits_undo_as_one_step_when_merged() {
    let mut ed = Editor::new(demo::demo_project());
    for v in [1.0, 2.0, 3.0] {
        ed.apply(&set_keys("scene-1", Some("maya"), "transform.rotation", vec![Key::new(10, v)]), Some("drag-key")).unwrap();
    }
    assert_eq!(ed.project.scenes[0].item("maya").unwrap().tracks["transform.rotation"].keys[0].value, json!(3.0));
    assert!(ed.undo());
    assert!(ed.project.scenes[0].item("maya").unwrap().tracks.is_empty());
}
