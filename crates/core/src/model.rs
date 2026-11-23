//! The project document. Plain data, serialised as JSON (camelCase), so a
//! project is human-readable, diff-able and easy to generate from scripts.

use crate::anim::Tracks;
use crate::geom::{Color, Ease};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Id = String;

/// Bump when the format changes; `migrate()` upgrades older files.
///
/// - 2: items and scenes can carry keyframe `tracks`, and characters can carry
///   named `outfits`. Version 1 files load unchanged (they have no tracks).
pub const FORMAT_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub scenes: Vec<Scene>,
    /// Imported media (audio). Shared via `Arc` so undo snapshots stay cheap.
    pub media: BTreeMap<Id, Arc<Media>>,
    /// Character parts from imported part packs that this project uses, copied
    /// in so the file renders anywhere (built-in parts are not stored).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parts: BTreeMap<Id, crate::pack::PartDef>,
    /// Poses saved in this project (on top of the pose library).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub poses: BTreeMap<Id, crate::pack::PoseDef>,
    /// Animations made in this project (for example chained from poses).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<Id, crate::pack::ActionDef>,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            format: "frameling".into(),
            version: FORMAT_VERSION,
            name: "Untitled".into(),
            width: 1280,
            height: 720,
            fps: 30,
            scenes: vec![Scene::new("s1", "Scene 1")],
            media: BTreeMap::new(),
            parts: BTreeMap::new(),
            poses: BTreeMap::new(),
            actions: BTreeMap::new(),
        }
    }
}

impl Project {
    pub fn from_json(s: &str) -> Result<Project, String> {
        let mut p: Project = serde_json::from_str(s).map_err(|e| format!("invalid project file: {e}"))?;
        p.migrate()?;
        Ok(p)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("project serialises")
    }
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("project serialises")
    }
    fn migrate(&mut self) -> Result<(), String> {
        if self.version > FORMAT_VERSION {
            return Err(format!(
                "this project was saved by a newer version (format {}); please update the app",
                self.version
            ));
        }
        self.version = FORMAT_VERSION;
        self.fps = self.fps.clamp(1, 120);
        // Hand-written files may have unsorted or duplicate keys.
        for s in &mut self.scenes {
            crate::anim::normalize_tracks(&mut s.tracks);
            for it in &mut s.items {
                crate::anim::normalize_tracks(&mut it.tracks);
            }
        }
        Ok(())
    }

    /// Total length in frames (scenes play back to back).
    pub fn total_frames(&self) -> u32 {
        self.scenes.iter().map(|s| s.duration.max(1)).sum()
    }
    /// Start frame of each scene on the global timeline.
    pub fn scene_starts(&self) -> Vec<u32> {
        let mut t = 0;
        self.scenes
            .iter()
            .map(|s| {
                let start = t;
                t += s.duration.max(1);
                start
            })
            .collect()
    }
    /// Which scene is showing at a global frame, and the local frame in it.
    pub fn locate(&self, frame: u32) -> Option<(usize, u32)> {
        let mut t = 0;
        for (i, s) in self.scenes.iter().enumerate() {
            let d = s.duration.max(1);
            if frame < t + d {
                return Some((i, frame - t));
            }
            t += d;
        }
        None
    }
    pub fn scene_index(&self, id: &str) -> Option<usize> {
        self.scenes.iter().position(|s| s.id == id)
    }
    pub fn scene(&self, id: &str) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Scene {
    pub id: Id,
    pub name: String,
    /// Length in frames.
    pub duration: u32,
    pub background: Background,
    pub camera: Camera,
    /// How this scene appears after the previous one.
    pub transition: Transition,
    /// Drawn back-to-front.
    pub items: Vec<Item>,
    pub audio: Vec<AudioClip>,
    /// Keyframe tracks for the scene itself (`camera.x`, `camera.y`, `camera.zoom`).
    #[serde(skip_serializing_if = "Tracks::is_empty")]
    pub tracks: Tracks,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::new("scene", "Scene")
    }
}

impl Scene {
    pub fn new(id: &str, name: &str) -> Scene {
        Scene {
            id: id.into(),
            name: name.into(),
            duration: 150,
            background: Background::default(),
            camera: Camera::default(),
            transition: Transition::default(),
            items: Vec::new(),
            audio: Vec::new(),
            tracks: Tracks::new(),
        }
    }
    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Background {
    /// Id of a background in a loaded asset pack.
    pub preset: String,
    /// Overrides for the preset's colour slots (`$0`, `$1`, ...).
    pub colors: Vec<Color>,
}

impl Default for Background {
    fn default() -> Self {
        Background { preset: "office".into(), colors: vec![] }
    }
}

/// Camera view: centre point in scene coordinates and zoom.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CamKey {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for CamKey {
    fn default() -> Self {
        CamKey { x: 640.0, y: 360.0, zoom: 1.0 }
    }
}

/// A camera move from `from` to `to` across the whole scene (or a still shot).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Camera {
    pub from: CamKey,
    pub to: CamKey,
    pub moving: bool,
    pub ease: Ease,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TransitionKind {
    #[default]
    Cut,
    Fade,
    FadeBlack,
    Slide,
    Zoom,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration: u32,
}

impl Default for Transition {
    fn default() -> Self {
        Transition { kind: TransitionKind::Cut, duration: 15 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Effect {
    #[default]
    None,
    Fade,
    Pop,
    SlideLeft,
    SlideRight,
    SlideUp,
}

/// Move from the item's position to (`x`, `y`) between local frames `start` and `end`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Motion {
    pub x: f32,
    pub y: f32,
    pub start: u32,
    pub end: u32,
    pub ease: Ease,
}

impl Default for Motion {
    fn default() -> Self {
        Motion { x: 0.0, y: 0.0, start: 0, end: 60, ease: Ease::InOut }
    }
}

/// Anything placed on the stage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub id: Id,
    pub name: String,
    /// First local frame the item is visible.
    pub start: u32,
    /// Last frame (exclusive). `None` = until the scene ends.
    pub end: Option<u32>,
    /// Anchor position in scene coordinates. Characters and props anchor at
    /// their feet / base; text anchors at its centre.
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub rotation: f32,
    pub flip: bool,
    pub opacity: f32,
    pub enter: Effect,
    pub exit: Effect,
    pub motion: Option<Motion>,
    pub locked: bool,
    pub kind: ItemKind,
    /// Keyframe tracks by path (`transform.x`, `bone.forearm_l.rot`, `look.skin`,
    /// `outfit`, ...). Keyed values override the fields above; see `anim::path`.
    #[serde(skip_serializing_if = "Tracks::is_empty")]
    pub tracks: Tracks,
}

impl Default for Item {
    fn default() -> Self {
        Item {
            id: String::new(),
            name: String::new(),
            start: 0,
            end: None,
            x: 640.0,
            y: 600.0,
            scale: 1.0,
            rotation: 0.0,
            flip: false,
            opacity: 1.0,
            enter: Effect::None,
            exit: Effect::None,
            motion: None,
            locked: false,
            kind: ItemKind::Prop(PropItem::default()),
            tracks: Tracks::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ItemKind {
    Character(CharacterItem),
    Prop(PropItem),
    Text(TextItem),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CharacterItem {
    pub look: Look,
    /// Id of an action in a loaded pack ("idle", "wave", ...).
    pub action: String,
    pub expression: Expression,
    /// Playback speed multiplier for the action.
    pub speed: f32,
    /// Play the walk cycle and face the direction of travel while moving.
    pub auto_walk: bool,
    /// Switch idle to the talk action while this character speaks.
    pub auto_talk: bool,
    /// Default voice for new lines (e.g. `supertonic:F1`, `supertonic:M2@de`, `system:Samantha`).
    pub voice: Option<String>,
    /// Default speaking speed for new lines (1.0 = normal).
    pub voice_speed: f32,
    /// Named costumes that the `outfit` track can switch to.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub outfits: BTreeMap<String, LookPatch>,
    /// Poses held over the animation, with blends in and out.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub poses: Vec<crate::poses::PoseStrip>,
    /// Which way the character faces (turnaround view).
    #[serde(skip_serializing_if = "is_default")]
    pub view: View,
    /// Turn to face the way it walks (side views; front or back when walking
    /// toward or away from the camera).
    pub auto_turn: bool,
}

/// The eight turnaround views. Right/left mean facing screen right/left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum View {
    #[default]
    Front,
    FrontRight,
    Right,
    BackRight,
    Back,
    BackLeft,
    Left,
    FrontLeft,
}

impl View {
    pub const ALL: [View; 8] = [View::Front, View::FrontRight, View::Right, View::BackRight, View::Back, View::BackLeft, View::Left, View::FrontLeft];
    /// Yaw in degrees: 0 front, 90 facing screen right, 180 back, -90 facing left.
    pub fn yaw(self) -> f32 {
        match self {
            View::Front => 0.0,
            View::FrontRight => 45.0,
            View::Right => 90.0,
            View::BackRight => 135.0,
            View::Back => 180.0,
            View::BackLeft => -135.0,
            View::Left => -90.0,
            View::FrontLeft => -45.0,
        }
    }
    /// The same view seen in a mirror (an item flipped horizontally).
    pub fn mirrored(self) -> View {
        match self {
            View::FrontRight => View::FrontLeft,
            View::FrontLeft => View::FrontRight,
            View::Right => View::Left,
            View::Left => View::Right,
            View::BackRight => View::BackLeft,
            View::BackLeft => View::BackRight,
            v => v,
        }
}
    }

impl Default for CharacterItem {
    fn default() -> Self {
        CharacterItem {
            look: Look::default(),
            action: "idle".into(),
            expression: Expression::Neutral,
            speed: 1.0,
            auto_walk: true,
            auto_talk: true,
            voice: None,
            voice_speed: 1.0,
            outfits: BTreeMap::new(),
            poses: Vec::new(),
            view: View::Front,
            auto_turn: true,
        }
    }
}

/// Character appearance, used by the character builder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Look {
    pub skin: Color,
    pub hair: Color,
    pub hair_style: HairStyle,
    pub top: Color,
    pub bottom: Color,
    pub shoes: Color,
    pub build: Build,
    pub accessory: Accessory,
    pub accent: Color,
    /// Drawing style: the full cartoon rig or a line-drawn stick figure.
    pub style: FigureStyle,
    /// Wear a dress (stick figures): `accent` is its colour.
    pub dress: bool,
    /// Face parts (cartoon figures; stick figures use their own simple face).
    #[serde(skip_serializing_if = "is_default")]
    pub eyes: EyeStyle,
    #[serde(skip_serializing_if = "is_default")]
    pub brows: BrowStyle,
    #[serde(skip_serializing_if = "is_default")]
    pub nose: NoseStyle,
    #[serde(skip_serializing_if = "is_default")]
    pub facial_hair: FacialHair,
    /// Wearable parts from part packs (hats, headphones, earrings, ...), drawn
    /// in order on top of the figure at their part's anchor.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wear: Vec<Wear>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// One wearable part on a character, with its colour slot overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Wear {
    pub part: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum EyeStyle {
    #[default]
    Round,
    Dots,
    Wide,
    Sleepy,
    Lashes,
    Narrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum BrowStyle {
    #[default]
    Normal,
    Thin,
    Thick,
    Arched,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum NoseStyle {
    #[default]
    Button,
    Dot,
    Round,
    Long,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FacialHair {
    #[default]
    None,
    Stubble,
    Moustache,
    Goatee,
    Beard,
    FullBeard,
}

/// A partial look: the fields an outfit (costume) changes. Unset fields keep
/// the character's own look.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct LookPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hair: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hair_style: Option<HairStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shoes: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessory: Option<Accessory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<FigureStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dress: Option<bool>,
}

impl LookPatch {
    /// Copy the set fields onto a look.
    pub fn apply(&self, l: &mut Look) {
        macro_rules! set {
            ($($f:ident),*) => { $(if let Some(v) = self.$f { l.$f = v; })* };
        }
        set!(skin, hair, hair_style, top, bottom, shoes, build, accessory, accent, style, dress);
    }
}

/// How a character is drawn. Both styles share poses, actions, expressions
/// and lip sync, so a stick figure can do everything a cartoon character can.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FigureStyle {
    #[default]
    Cartoon,
    Stick,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            skin: Color::rgb(0xf1, 0xc2, 0x9a),
            hair: Color::rgb(0x3b, 0x2a, 0x20),
            hair_style: HairStyle::Short,
            top: Color::rgb(0x4f, 0x7c, 0xff),
            bottom: Color::rgb(0x2f, 0x3a, 0x56),
            shoes: Color::rgb(0x2a, 0x22, 0x1e),
            build: Build::Regular,
            accessory: Accessory::None,
            accent: Color::rgb(0xe8, 0x4a, 0x5f),
            style: FigureStyle::Cartoon,
            dress: false,
            eyes: EyeStyle::Round,
            brows: BrowStyle::Normal,
            nose: NoseStyle::Button,
            facial_hair: FacialHair::None,
            wear: Vec::new(),
        }
    }
}

impl Look {
    /// A stick figure: black lines, white head. `female` adds long hair and a dress.
    pub fn stick(female: bool) -> Look {
        Look {
            skin: Color::WHITE,
            hair: Color::rgb(0x1f, 0x22, 0x33),
            hair_style: if female { HairStyle::Long } else { HairStyle::Bald },
            top: Color::rgb(0x1f, 0x22, 0x33),
            bottom: Color::rgb(0x1f, 0x22, 0x33),
            shoes: Color::rgb(0x1f, 0x22, 0x33),
            build: Build::Regular,
            accessory: Accessory::None,
            accent: Color::rgb(0xe8, 0x4a, 0x5f),
            style: FigureStyle::Stick,
            dress: female,
            ..Look::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum HairStyle {
    Bald,
    #[default]
    Short,
    Spiky,
    Long,
    Bun,
    Curly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Build {
    Slim,
    #[default]
    Regular,
    Broad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Accessory {
    #[default]
    None,
    Glasses,
    Tie,
    Cap,
    Bow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Expression {
    #[default]
    Neutral,
    Happy,
    Sad,
    Angry,
    Surprised,
    Thinking,
    Wink,
    Worried,
    Confused,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropItem {
    /// Id of a prop in a loaded pack.
    pub prop: String,
    /// Overrides for the prop's colour slots.
    pub colors: Vec<Color>,
}

impl Default for PropItem {
    fn default() -> Self {
        PropItem { prop: "plant".into(), colors: vec![] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TextStyle {
    #[default]
    Bubble,
    Thought,
    Caption,
    Title,
    Plain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Tail {
    #[default]
    BottomLeft,
    BottomRight,
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextItem {
    pub text: String,
    pub style: TextStyle,
    pub size: f32,
    pub color: Color,
    pub fill: Color,
    pub tail: Tail,
    /// Wrap width in scene pixels.
    pub width: f32,
    /// Typewriter reveal (characters per second, 0 = off).
    pub typewriter: f32,
}

impl Default for TextItem {
    fn default() -> Self {
        TextItem {
            text: "Hello!".into(),
            style: TextStyle::Bubble,
            size: 28.0,
            color: Color::INK,
            fill: Color::WHITE,
            tail: Tail::BottomLeft,
            width: 320.0,
            typewriter: 0.0,
        }
    }
}

/// A sound placed in a scene. If `speaker` names a character, that character
/// lip-syncs to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioClip {
    pub id: Id,
    pub media: Id,
    /// Local frame the clip starts at.
    pub start: u32,
    pub volume: f32,
    pub speaker: Option<Id>,
    /// Frames to skip at the beginning of the media.
    pub trim_start: u32,
    /// Max frames to play (`None` = to the end of the media).
    pub length: Option<u32>,
}

impl Default for AudioClip {
    fn default() -> Self {
        AudioClip { id: String::new(), media: String::new(), start: 0, volume: 1.0, speaker: None, trim_start: 0, length: None }
    }
}

impl AudioClip {
    /// Frames this clip actually plays for.
    pub fn play_frames(&self, media: &Media) -> u32 {
        let avail = media.duration_frames.saturating_sub(self.trim_start);
        self.length.map_or(avail, |l| l.min(avail))
    }
}

/// An imported media file, embedded in the project (base64) so a project is a
/// single portable file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Media {
    pub id: Id,
    pub name: String,
    pub mime: String,
    /// Base64 file contents.
    pub data: String,
    /// Length in frames at the project fps.
    pub duration_frames: u32,
    /// Loudness envelope, one value in 0..=1 per frame, used for lip sync.
    /// Computed when the file is imported, so playback and export agree.
    pub envelope: Vec<f32>,
    /// Where the sound came from (lets the editor re-generate a typed line).
    pub source: Option<MediaSource>,
}

/// Origin of a sound.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MediaSource {
    /// Typed line spoken by a text-to-speech voice.
    Tts { text: String, voice: String, speed: f32 },
    /// Recorded with the microphone.
    Recording,
    /// Imported file.
    File,
}

impl Default for Media {
    fn default() -> Self {
        Media { id: String::new(), name: String::new(), mime: String::new(), data: String::new(), duration_frames: 0, envelope: vec![], source: None }
    }
}
