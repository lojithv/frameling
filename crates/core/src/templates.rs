//! Ready-made projects to start from.
//!
//! Each template is laid out for the frame shape it is made in: the world is
//! always 1280x720 design units, and a vertical or square frame shows a slice
//! of it (see `eval::view_scale`), so characters, titles and props are placed
//! inside the slice that will be visible. Templates come with music, styled
//! subtitles and a voice per character, so all that's left is typing lines.

use crate::geom::{Color, Ease};
use crate::model::*;

/// One template in the picker.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Shape it looks best in ("vertical", "wide", "square").
    pub best: &'static str,
}

pub fn catalog() -> Vec<TemplateInfo> {
    vec![
        TemplateInfo { id: "chat", name: "Two-person chat", description: "Two friends face each other and talk, with pop subtitles and a chill beat.", best: "vertical" },
        TemplateInfo { id: "explainer", name: "Story explainer", description: "A hook title, a host who explains with a whiteboard, and a follow-for-more ending.", best: "vertical" },
        TemplateInfo { id: "reaction", name: "Reaction", description: "A space for the clip up top and a reacting character below, with a zoom punch.", best: "vertical" },
        TemplateInfo { id: "podcast", name: "Animated podcast", description: "Two hosts at a desk in a studio, with a show title and a quiet music bed.", best: "wide" },
        TemplateInfo { id: "news", name: "News flash", description: "An anchor at the desk, a breaking-news banner and a news theme.", best: "wide" },
        TemplateInfo { id: "stick-story", name: "Stick figure story", description: "A sketchbook story in three scenes with stick figures.", best: "square" },
    ]
}

/// Frame size for a shape name.
pub fn size(shape: &str) -> (u32, u32) {
    match shape {
        "vertical" => (1080, 1920),
        "square" => (1080, 1080),
        "wide1080" => (1920, 1080),
        _ => (1280, 720),
    }
}

/// The part of the world a frame shows with the camera at home, in design units.
struct View {
    w: f32,
    h: f32,
}

impl View {
    fn of(p: &Project) -> View {
        let s = crate::eval::view_scale(p);
        View { w: p.width as f32 / s, h: p.height as f32 / s }
    }
    fn tall(&self) -> bool {
        self.h > self.w * 1.2
    }
    /// x of a point `f` across the view (-0.5 left edge, 0.5 right edge).
    #[allow(dead_code)]
    fn x(&self, f: f32) -> f32 {
        640.0 + f * self.w
    }
    /// y of a point `f` down the view (0 top, 1 bottom).
    fn y(&self, f: f32) -> f32 {
        360.0 - self.h / 2.0 + f * self.h
    }
}

fn look(skin: (u8, u8, u8), hair: (u8, u8, u8), style: HairStyle, top: (u8, u8, u8), mouth: MouthSet) -> Look {
    Look {
        skin: Color::rgb(skin.0, skin.1, skin.2),
        hair: Color::rgb(hair.0, hair.1, hair.2),
        hair_style: style,
        top: Color::rgb(top.0, top.1, top.2),
        mouth,
        ..Default::default()
    }
}

fn host_a() -> Look {
    Look { accessory: Accessory::Glasses, ..look((0xc6, 0x8b, 0x62), (0x23, 0x1a, 0x16), HairStyle::Long, (0xf2, 0x6b, 0x5b), MouthSet::Lips) }
}
fn host_b() -> Look {
    Look { build: Build::Broad, ..look((0xf3, 0xc9, 0xa5), (0xd9, 0x9a, 0x3e), HairStyle::Spiky, (0x3f, 0xae, 0x6a), MouthSet::Classic) }
}
fn host_c() -> Look {
    look((0x8d, 0x5a, 0x3b), (0x1b, 0x13, 0x11), HairStyle::Curly, (0x7c, 0x5c, 0xff), MouthSet::Classic)
}

fn character(id: &str, name: &str, x: f32, y: f32, scale: f32, look: Look, voice: &str) -> Item {
    Item {
        id: id.into(),
        name: name.into(),
        x,
        y,
        scale,
        kind: ItemKind::Character(CharacterItem { look, voice: Some(voice.into()), ..Default::default() }),
        ..Default::default()
    }
}

fn set_char(it: &mut Item, f: impl FnOnce(&mut CharacterItem)) {
    if let ItemKind::Character(c) = &mut it.kind {
        f(c);
    }
}

fn prop(id: &str, prop: &str, x: f32, y: f32, scale: f32) -> Item {
    Item { id: id.into(), name: prop.into(), x, y, scale, kind: ItemKind::Prop(PropItem { prop: prop.into(), colors: vec![] }), ..Default::default() }
}

/// Text pinned to the frame (it ignores the camera).
#[allow(clippy::too_many_arguments)]
fn title(id: &str, s: &str, x: f32, y: f32, size: f32, width: f32, style: TextStyle, reveal: Reveal) -> Item {
    let mut t = TextItem { text: s.into(), style, size, width, font: Font::Heavy, pinned: true, reveal, reveal_speed: 6.0, ..Default::default() };
    match style {
        TextStyle::Title | TextStyle::Plain => {
            t.color = Color::WHITE;
            t.stroke = Color::rgb(0x14, 0x16, 0x1f);
            t.stroke_width = size * 0.08;
        }
        TextStyle::Caption => {
            t.color = Color::WHITE;
            t.fill = Color::rgba(0x14, 0x17, 0x26, 0xd8);
        }
        _ => {}
    }
    Item { id: id.into(), name: "Title".into(), x, y, enter: Effect::Pop, kind: ItemKind::Text(t), ..Default::default() }
}

fn with_music(p: &mut Project, sound: &str, volume: f32) {
    if let Some(m) = crate::sfx::media(sound, p.fps) {
        p.music = Some(Music { media: m.id.clone(), volume, ..Default::default() });
        p.media.insert(m.id.clone(), std::sync::Arc::new(m));
    }
}

fn captions(p: &mut Project, style: CaptionStyle) {
    let mut c = crate::captions::defaults_for(p.width, p.height);
    c.enabled = true;
    c.style = style;
    p.captions = c;
}

/// A zoom punch on the scene camera at `frame` (in quickly, ease back out).
fn punch(s: &mut Scene, frame: u32, base: f32, amount: f32, fps: u32) {
    let keys = [(frame.saturating_sub(1), base, crate::anim::Interp::Constant), (frame, base * amount, crate::anim::Interp::EaseInOut), (frame + fps, base, crate::anim::Interp::Linear)];
    let track = s.tracks.entry("camera.zoom".into()).or_default();
    for (f, v, interp) in keys {
        track.keys.push(crate::anim::Key { frame: f, value: serde_json::Value::from(v as f64), interp });
    }
    crate::anim::normalize_tracks(&mut s.tracks);
}

/// Build a template for a frame shape ("vertical", "wide", "square").
pub fn build(id: &str, shape: &str) -> Option<Project> {
    build_with(id, shape, true)
}

/// `music` = false skips generating the music (quick previews).
pub fn build_with(id: &str, shape: &str, music: bool) -> Option<Project> {
    let (width, height) = size(shape);
    let mut p = Project { width, height, ..Default::default() };
    let v = View::of(&p);
    let preview = !music;
    let fps = p.fps;
    let tall = v.tall();
    let ground = 640.0;
    // Characters a little bigger in a narrow frame, where they have the room.
    let k = if tall { 1.0 } else { 0.95 };
    match id {
        "chat" => {
            p.name = "Two-person chat".into();
            let mut s = Scene::new("s1", "Chat");
            s.duration = 8 * fps;
            s.background.preset = "living-room".into();
            let d = (v.w * 0.26).min(230.0);
            let mut a = character("ava", "Ava", 640.0 - d, ground, k, host_a(), "supertonic:F1");
            let mut b = character("ben", "Ben", 640.0 + d, ground, k, host_b(), "supertonic:M2");
            set_char(&mut a, |c| {
                c.view = crate::model::View::FrontRight;
                c.expression = Expression::Happy;
            });
            set_char(&mut b, |c| c.view = crate::model::View::FrontLeft);
            s.items = vec![
                prop("sofa", "sofa", 640.0, ground, 0.9),
                prop("lamp", "lamp", 640.0 + d + v.w * 0.2, ground, 0.9),
                a,
                b,
                title("title", "Two friends talk", 640.0, v.y(0.1), if tall { 40.0 } else { 46.0 }, v.w * 0.86, TextStyle::Title, Reveal::PopWords),
            ];
            s.items[4].end = Some(3 * fps);
            s.items[4].exit = Effect::Fade;
            p.scenes = vec![s];
            captions(&mut p, CaptionStyle::Pop);
            if !preview { with_music(&mut p, "music-lofi", 0.35); }
        }
        "explainer" => {
            p.name = "Story explainer".into();
            let host = host_c();
            let hx = if tall { 640.0 } else { 640.0 - v.w * 0.25 };
            let mut s1 = Scene::new("s1", "Hook");
            s1.duration = 3 * fps;
            s1.background.preset = "classroom".into();
            let mut h1 = character("host", "Sam", hx, ground, k, host.clone(), "supertonic:M1");
            set_char(&mut h1, |c| {
                c.action = "point".into();
                c.expression = Expression::Surprised;
            });
            s1.items = vec![h1, title("hook", "Did you know...?", 640.0, v.y(0.16), if tall { 52.0 } else { 64.0 }, v.w * 0.9, TextStyle::Title, Reveal::PopWords)];
            punch(&mut s1, fps / 2, 1.0, 1.12, fps);

            let mut s2 = Scene::new("s2", "Explain");
            s2.duration = 6 * fps;
            s2.background.preset = "classroom".into();
            s2.transition = Transition { kind: TransitionKind::Slide, duration: 12 };
            let mut h2 = character("host2", "Sam", if tall { 640.0 + v.w * 0.18 } else { hx }, ground, k, host.clone(), "supertonic:M1");
            set_char(&mut h2, |c| c.action = "talk".into());
            let board_x = if tall { 640.0 - v.w * 0.2 } else { 640.0 + v.w * 0.18 };
            s2.items = vec![
                prop("board", "whiteboard", board_x, 520.0, if tall { 0.7 } else { 1.0 }),
                h2,
                title("point", "Here's the idea", 640.0, v.y(0.12), if tall { 40.0 } else { 44.0 }, v.w * 0.86, TextStyle::Caption, Reveal::RiseWords),
            ];

            let mut s3 = Scene::new("s3", "Outro");
            s3.duration = 3 * fps;
            s3.background.preset = "studio".into();
            s3.transition = Transition { kind: TransitionKind::Zoom, duration: 12 };
            let mut h3 = character("host3", "Sam", 640.0, ground, k, host, "supertonic:M1");
            set_char(&mut h3, |c| {
                c.action = "wave".into();
                c.expression = Expression::Happy;
            });
            h3.enter = Effect::Pop;
            s3.items = vec![h3, title("follow", "Follow for more!", 640.0, v.y(0.16), if tall { 48.0 } else { 60.0 }, v.w * 0.9, TextStyle::Title, Reveal::Bounce)];
            p.scenes = vec![s1, s2, s3];
            captions(&mut p, CaptionStyle::Karaoke);
            if !preview { with_music(&mut p, "music-sunny", 0.3); }
        }
        "reaction" => {
            p.name = "Reaction".into();
            let mut s = Scene::new("s1", "Reaction");
            s.duration = 6 * fps;
            s.background.preset = "studio".into();
            // The clip area: the top half of a tall frame, the left of a wide one.
            let (cx, cy, cw, size) = if tall { (640.0, v.y(0.27), v.w * 0.88, 34.0) } else { (640.0 - v.w * 0.2, 300.0, v.w * 0.5, 36.0) };
            let mut frame = title("clip", "Your clip here\n(import an image or video still)", cx, cy, size, cw, TextStyle::Caption, Reveal::None);
            frame.enter = Effect::Zoom;
            let mut r = character("me", "Me", if tall { 640.0 } else { 640.0 + v.w * 0.25 }, ground + if tall { 60.0 } else { 0.0 }, if tall { 1.15 } else { 1.0 }, host_b(), "supertonic:M2");
            set_char(&mut r, |c| {
                c.expression = Expression::Surprised;
                c.action = "shrug".into();
            });
            s.items = vec![frame, r];
            punch(&mut s, fps * 2, 1.0, 1.15, fps);
            punch(&mut s, fps * 4, 1.0, 1.1, fps);
            p.scenes = vec![s];
            captions(&mut p, CaptionStyle::Boxed);
            if !preview { with_music(&mut p, "music-chip", 0.3); }
        }
        "podcast" => {
            p.name = "Animated podcast".into();
            let mut s = Scene::new("s1", "Episode");
            s.duration = 10 * fps;
            s.background.preset = "studio".into();
            let d = (v.w * 0.24).min(250.0);
            let mut a = character("host1", "Ava", 640.0 - d, ground, k, host_a(), "supertonic:F2");
            let mut b = character("host2", "Leo", 640.0 + d, ground, k, host_c(), "supertonic:M3");
            set_char(&mut a, |c| c.view = crate::model::View::FrontRight);
            set_char(&mut b, |c| c.view = crate::model::View::FrontLeft);
            let mut lower = title("show", "EP. 1  ·  Your show name", 640.0, v.y(if tall { 0.12 } else { 0.1 }), if tall { 30.0 } else { 30.0 }, v.w * 0.8, TextStyle::Caption, Reveal::None);
            lower.enter = Effect::SlideLeft;
            if let ItemKind::Text(t) = &mut lower.kind {
                t.fill = Color::rgba(0x7c, 0x5c, 0xff, 0xe6);
            }
            s.items = vec![a, b, prop("desk", "desk", 640.0, ground + 10.0, if tall { 0.85 } else { 1.1 }), prop("coffee", "coffee", 640.0 - 40.0, 500.0, 0.7), lower];
            p.scenes = vec![s];
            captions(&mut p, CaptionStyle::Clean);
            if !preview { with_music(&mut p, "music-lofi", 0.18); }
        }
        "news" => {
            p.name = "News flash".into();
            let mut s = Scene::new("s1", "Breaking");
            s.duration = 8 * fps;
            s.background.preset = "office".into();
            let mut anchor = character("anchor", "Anchor", if tall { 640.0 } else { 640.0 - v.w * 0.12 }, ground, k, Look { accessory: Accessory::Tie, ..host_c() }, "supertonic:M1");
            set_char(&mut anchor, |c| c.action = "talk".into());
            let mut banner = title("breaking", "BREAKING NEWS", 640.0, v.y(if tall { 0.76 } else { 0.8 }), if tall { 40.0 } else { 44.0 }, v.w * 0.9, TextStyle::Caption, Reveal::None);
            banner.enter = Effect::SlideRight;
            if let ItemKind::Text(t) = &mut banner.kind {
                t.fill = Color::rgb(0xd6, 0x28, 0x3c);
                t.emphasis = Emphasis::Pulse;
            }
            let headline = title("headline", "Your headline goes here", 640.0, v.y(if tall { 0.83 } else { 0.9 }), if tall { 28.0 } else { 30.0 }, v.w * 0.86, TextStyle::Caption, Reveal::Typewriter);
            s.items = vec![anchor, prop("desk", "desk", if tall { 640.0 } else { 640.0 - v.w * 0.12 }, ground + 10.0, 1.1), banner, headline];
            s.camera.from.zoom = if tall { 1.0 } else { 1.05 };
            p.scenes = vec![s];
            captions(&mut p, CaptionStyle::Boxed);
            p.captions.y = if tall { 0.2 } else { 0.14 };
            p.captions.size = if tall { 40.0 } else { 34.0 };
            if !preview { with_music(&mut p, "music-news", 0.25); }
        }
        "stick-story" => {
            p.name = "Stick figure story".into();
            let d = (v.w * 0.22).min(220.0);
            let mut s1 = Scene::new("s1", "Once upon a time");
            s1.duration = 4 * fps;
            s1.background.preset = "stick-hills".into();
            let mut kid = character("kid", "Kid", 640.0 - d, ground, k, Look::stick(false), "supertonic:M4");
            set_char(&mut kid, |c| c.action = "wave".into());
            s1.items = vec![prop("tree", "stick-tree", 640.0 + d, ground, 0.9), kid, title("once", "Once upon a time...", 640.0, v.y(if tall { 0.2 } else { 0.12 }), 44.0, v.w * 0.9, TextStyle::Plain, Reveal::Typewriter)];
            if let ItemKind::Text(t) = &mut s1.items[2].kind {
                t.color = Color::INK;
                t.stroke_width = 0.0;
                t.font = Font::Rounded;
            }
            let mut s2 = Scene::new("s2", "Meeting");
            s2.duration = 6 * fps;
            s2.background.preset = "stick-paper".into();
            s2.transition = Transition { kind: TransitionKind::Fade, duration: 12 };
            let mut a = character("kid2", "Kid", 640.0 - d, ground, k, Look::stick(false), "supertonic:M4");
            let mut b = character("friend", "Friend", 640.0 + d, ground, k, Look::stick(true), "supertonic:F3");
            set_char(&mut a, |c| c.view = crate::model::View::FrontRight);
            set_char(&mut b, |c| {
                c.view = crate::model::View::FrontLeft;
                c.expression = Expression::Happy;
            });
            s2.items = vec![a, b];
            let mut s3 = Scene::new("s3", "The end");
            s3.duration = 3 * fps;
            s3.background.preset = "stick-stage".into();
            s3.transition = Transition { kind: TransitionKind::FadeBlack, duration: 14 };
            let mut c1 = character("kid3", "Kid", 640.0 - d * 0.6, ground, k, Look::stick(false), "supertonic:M4");
            let mut c2 = character("friend3", "Friend", 640.0 + d * 0.6, ground, k, Look::stick(true), "supertonic:F3");
            set_char(&mut c1, |c| c.action = "cheer".into());
            set_char(&mut c2, |c| c.action = "cheer".into());
            s3.items = vec![c1, c2, title("end", "The End", 640.0, v.y(0.16), 60.0, v.w * 0.8, TextStyle::Plain, Reveal::Bounce)];
            if let ItemKind::Text(t) = &mut s3.items[2].kind {
                t.color = Color::INK;
                t.stroke_width = 0.0;
            }
            p.scenes = vec![s1, s2, s3];
            captions(&mut p, CaptionStyle::Clean);
            p.captions.color = Color::INK;
            p.captions.stroke = Color::WHITE;
            if !preview { with_music(&mut p, "music-mystery", 0.3); }
        }
        _ => return None,
    }
    let _ = Ease::InOut;
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_and_keeps_its_cast_in_frame() {
        let lib = crate::pack::Library::builtin();
        for t in catalog() {
            for shape in ["vertical", "wide", "square"] {
                let p = build(t.id, shape).unwrap();
                assert!(p.music.is_some() && p.captions.enabled, "{} {shape}", t.id);
                for (si, s) in p.scenes.iter().enumerate() {
                    let (_, hits) = crate::eval::evaluate_scene(&p, lib, si, s.duration / 2);
                    for (h, it) in hits.iter().zip(&s.items) {
                        if !matches!(it.kind, ItemKind::Character(_)) || h.bounds.is_empty() {
                            continue;
                        }
                        let cx = h.bounds.x + h.bounds.w / 2.0;
                        assert!(cx > 0.0 && cx < p.width as f32, "{} {shape}: {} is off screen at x {cx}", t.id, it.id);
                    }
                }
            }
        }
    }
}
