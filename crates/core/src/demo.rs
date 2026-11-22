//! A small demo project, used as the "New from template" starter and in tests.

use crate::geom::{Color, Ease};
use crate::model::*;

fn character(id: &str, name: &str, x: f32, look: Look, action: &str, expression: Expression) -> Item {
    Item {
        id: id.into(),
        name: name.into(),
        x,
        y: 640.0,
        scale: 0.95,
        kind: ItemKind::Character(CharacterItem { look, action: action.into(), expression, ..Default::default() }),
        ..Default::default()
    }
}

fn prop(id: &str, prop: &str, x: f32, y: f32, scale: f32) -> Item {
    Item {
        id: id.into(),
        name: prop.into(),
        x,
        y,
        scale,
        kind: ItemKind::Prop(PropItem { prop: prop.into(), colors: vec![] }),
        ..Default::default()
    }
}

fn text(id: &str, s: &str, style: TextStyle, x: f32, y: f32, start: u32, end: Option<u32>) -> Item {
    let mut t = TextItem { text: s.into(), style, ..Default::default() };
    match style {
        TextStyle::Title => {
            t.size = 64.0;
            t.color = Color::WHITE;
            t.width = 1000.0;
        }
        TextStyle::Caption => {
            t.size = 30.0;
            t.color = Color::WHITE;
            t.fill = Color::rgba(0x14, 0x17, 0x26, 0xcc);
            t.width = 900.0;
        }
        _ => {}
    }
    Item { id: id.into(), name: "Text".into(), x, y, start, end, enter: Effect::Pop, kind: ItemKind::Text(t), ..Default::default() }
}

pub fn demo_project() -> Project {
    let maya = Look {
        skin: Color::rgb(0xc6, 0x8b, 0x62),
        hair: Color::rgb(0x23, 0x1a, 0x16),
        hair_style: HairStyle::Long,
        top: Color::rgb(0xf2, 0x6b, 0x5b),
        bottom: Color::rgb(0x2f, 0x3a, 0x56),
        accessory: Accessory::Glasses,
        ..Default::default()
    };
    let leo = Look {
        skin: Color::rgb(0xf3, 0xc9, 0xa5),
        hair: Color::rgb(0xd9, 0x9a, 0x3e),
        hair_style: HairStyle::Spiky,
        top: Color::rgb(0x3f, 0xae, 0x6a),
        bottom: Color::rgb(0x45, 0x4d, 0x72),
        build: Build::Broad,
        accessory: Accessory::Tie,
        accent: Color::rgb(0x4f, 0x7c, 0xff),
        ..Default::default()
    };

    let mut s1 = Scene::new("scene-1", "Office hello");
    s1.duration = 120;
    s1.background.preset = "office".into();
    s1.items = vec![
        prop("desk", "desk", 900.0, 640.0, 1.0),
        prop("laptop", "laptop", 870.0, 490.0, 1.0),
        prop("coffee", "coffee", 960.0, 490.0, 0.8),
        prop("plant", "plant", 1180.0, 640.0, 1.0),
        character("maya", "Maya", 380.0, maya.clone(), "wave", Expression::Happy),
        character("leo", "Leo", 640.0, leo.clone(), "idle", Expression::Neutral),
        text("hi", "Hi! Welcome to our very first cartoon.", TextStyle::Bubble, 330.0, 170.0, 10, Some(110)),
    ];
    if let ItemKind::Text(t) = &mut s1.items[6].kind {
        t.tail = Tail::BottomLeft;
        t.typewriter = 30.0;
    }

    let mut s2 = Scene::new("scene-2", "Walk in the park");
    s2.duration = 120;
    s2.background.preset = "park".into();
    s2.transition = Transition { kind: TransitionKind::Slide, duration: 18 };
    s2.camera = Camera {
        from: CamKey { x: 640.0, y: 360.0, zoom: 1.0 },
        to: CamKey { x: 760.0, y: 400.0, zoom: 1.15 },
        moving: true,
        ease: Ease::InOut,
    };
    let mut walker = character("leo2", "Leo", 180.0, leo, "idle", Expression::Happy);
    walker.motion = Some(Motion { x: 820.0, y: 640.0, start: 0, end: 80, ease: Ease::InOut });
    s2.items = vec![
        prop("tree", "tree", 1040.0, 600.0, 1.0),
        prop("bench", "bench", 620.0, 600.0, 1.0),
        prop("bush", "bush", 140.0, 610.0, 1.0),
        walker,
        text("cap", "Leo takes a walk in the park.", TextStyle::Caption, 640.0, 680.0, 6, None),
    ];

    let mut s3 = Scene::new("scene-3", "The end");
    s3.duration = 90;
    s3.background.preset = "night".into();
    s3.transition = Transition { kind: TransitionKind::Fade, duration: 15 };
    let mut m = character("maya3", "Maya", 640.0, maya, "cheer", Expression::Happy);
    m.enter = Effect::Pop;
    s3.items = vec![m, text("end", "The End", TextStyle::Title, 640.0, 150.0, 10, None)];

    Project { name: "Demo".into(), scenes: vec![s1, s2, s3], ..Default::default() }
}

/// A blank project with one empty scene.
pub fn blank_project() -> Project {
    Project::default()
}
