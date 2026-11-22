//! The project document. Plain data, serialised as JSON (camelCase), so a
//! project is human-readable, diff-able and easy to generate from scripts.

use crate::geom::{Color, Ease};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Id = String;

/// Bump when the format changes; `migrate()` upgrades older files.
pub const FORMAT_VERSION: u32 = 1;

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
