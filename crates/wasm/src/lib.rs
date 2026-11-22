//! The engine as seen from JavaScript.
//!
//! The editor UI keeps no document state of its own: it sends commands here,
//! reads the project back as JSON, and asks for rendered frames. The desktop
//! app and the web app use this exact same module.

use frameling_core::{
    audio_events, demo, eval, evaluate, Command, DisplayList, Editor, Item, ItemKind, Library, Project, Scene,
};
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen]
pub struct Engine {
    editor: Editor,
    lib: Library,
    last_hits: String,
}

/// Convert premultiplied RGBA to straight alpha (what `ImageData` expects).
fn unpremultiply(mut px: Vec<u8>) -> Vec<u8> {
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    px
}

#[wasm_bindgen]
impl Engine {
    /// Create an engine with a blank project.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Engine {
        Engine { editor: Editor::new(demo::blank_project()), lib: Library::builtin().clone(), last_hits: "[]".into() }
    }

    /// Replace the document with the demo project.
    pub fn load_demo(&mut self) {
        self.editor.load(demo::demo_project());
    }

    pub fn load_blank(&mut self) {
        self.editor.load(demo::blank_project());
    }

    /// Open a project from JSON. Clears undo history.
    pub fn load(&mut self, json: &str) -> Result<(), JsError> {
        let p = Project::from_json(json).map_err(err)?;
        self.editor.load(p);
        Ok(())
    }

    pub fn project_json(&self) -> String {
        self.editor.project.to_json()
    }

    /// Project JSON without embedded media payloads (cheap to send to the UI on every change).
    pub fn project_view_json(&self) -> String {
        let mut p = self.editor.project.clone();
        for m in p.media.values_mut() {
            let mut lite = (**m).clone();
            lite.data = String::new();
            lite.envelope = Vec::new();
            *m = std::sync::Arc::new(lite);
        }
        p.to_json()
    }

    /// Base64 data of one media file.
    pub fn media_data(&self, id: &str) -> Option<String> {
        self.editor.project.media.get(id).map(|m| m.data.clone())
    }

    /// Apply a JSON command. `merge` groups a gesture into one undo step.
    pub fn apply(&mut self, cmd: &str, merge: Option<String>) -> Result<(), JsError> {
        let c: Command = serde_json::from_str(cmd).map_err(err)?;
        self.editor.apply(&c, merge.as_deref()).map_err(err)
    }

    pub fn undo(&mut self) -> bool {
        self.editor.undo()
    }
    pub fn redo(&mut self) -> bool {
        self.editor.redo()
    }
    pub fn seal(&mut self) {
        self.editor.seal()
    }
    pub fn can_undo(&self) -> bool {
        self.editor.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.editor.can_redo()
    }
    pub fn is_dirty(&self) -> bool {
        self.editor.dirty
    }
    pub fn mark_saved(&mut self) {
        self.editor.dirty = false;
    }

    /// Library catalogue (props, backgrounds, actions, packs) as JSON.
    pub fn catalog(&self) -> String {
        self.lib.catalog().to_string()
    }

    /// Load an extra asset pack. Returns its id.
    pub fn add_pack(&mut self, json: &str) -> Result<String, JsError> {
        self.lib.add_pack_json(json).map_err(err)
    }

    pub fn total_frames(&self) -> u32 {
        self.editor.project.total_frames()
    }

    /// `[sceneIndex, localFrame]` for a global frame.
    pub fn locate(&self, frame: u32) -> Vec<u32> {
        match self.editor.project.locate(frame) {
            Some((i, l)) => vec![i as u32, l],
            None => vec![],
        }
    }

    /// Render one scene at a local frame (the editing view). Item bounds for
    /// hit-testing are available afterwards from `last_hits()`.
    pub fn render_scene(&mut self, scene: usize, local: u32, width: u32, height: u32) -> Vec<u8> {
        let (dl, hits) = eval::evaluate_scene(&self.editor.project, &self.lib, scene, local);
        self.last_hits = serde_json::to_string(&hits).unwrap_or_else(|_| "[]".into());
        frameling_render::render(&dl, width, height)
    }

    pub fn last_hits(&self) -> String {
        self.last_hits.clone()
    }

    /// Render a global frame (playback and export), with transitions.
    pub fn render_frame(&self, frame: u32, width: u32, height: u32) -> Vec<u8> {
        frameling_render::render(&evaluate(&self.editor.project, &self.lib, frame), width, height)
    }

    /// Audio placements for playback/export, as JSON.
    pub fn audio_events(&self) -> String {
        serde_json::to_string(&audio_events(&self.editor.project)).unwrap_or_default()
    }

    /// Render a single item (JSON) centred on a transparent canvas: library
    /// thumbnails and drag previews.
    pub fn render_item_preview(&self, item_json: &str, width: u32, height: u32, frame: u32) -> Result<Vec<u8>, JsError> {
        let mut item: Item = serde_json::from_str(item_json).map_err(err)?;
        item.x = 0.0;
        item.y = 0.0;
        item.scale = 1.0;
        item.start = 0;
        item.end = None;
        item.motion = None;
        if item.id.is_empty() {
            item.id = "preview".into();
        }
        let mut p = Project::default();
        let mut s = Scene::new("preview", "preview");
        s.background.preset = String::new();
        s.background.colors = vec![frameling_core::Color::rgba(0, 0, 0, 0)];
        s.items.push(item);
        p.scenes = vec![s];
        let (dl, hits) = eval::evaluate_scene(&p, &self.lib, 0, frame);
        let b = hits.first().map(|h| h.bounds).unwrap_or(frameling_core::Rect { x: -50.0, y: -100.0, w: 100.0, h: 100.0 });
        // The scene camera centres (640,360) on screen; undo that and fit the bounds.
        let pad = 0.08;
        let s = ((width as f32 * (1.0 - 2.0 * pad)) / b.w.max(1.0)).min((height as f32 * (1.0 - 2.0 * pad)) / b.h.max(1.0));
        let cx = b.x + b.w / 2.0;
        let cy = b.y + b.h / 2.0;
        let fit = frameling_core::Affine::translate(width as f32 / 2.0, height as f32 / 2.0)
            .then(&frameling_core::Affine::scale(s, s))
            .then(&frameling_core::Affine::translate(-cx, -cy));
        // Drop the background op (first child of the camera group) for transparency.
        let mut ops = dl.ops;
        if let Some(frameling_core::Op::Group { children, .. }) = ops.first_mut() {
            if !children.is_empty() {
                children.remove(0);
            }
        }
        let fitted = DisplayList {
            width,
            height,
            background: None,
            ops: vec![frameling_core::Op::group(fit, 1.0, ops)],
        };
        Ok(unpremultiply(frameling_render::render(&fitted, width, height)))
    }

    /// Render a background preset as a thumbnail.
    pub fn render_background_preview(&self, id: &str, width: u32, height: u32) -> Vec<u8> {
        let mut p = Project::default();
        let mut s = Scene::new("bg", "bg");
        s.background.preset = id.into();
        p.scenes = vec![s];
        let (dl, _) = eval::evaluate_scene(&p, &self.lib, 0, 0);
        frameling_render::render(&dl, width, height)
    }

    /// The kind of an item as a string, handy for the UI.
    pub fn item_kind(&self, scene: usize, id: &str) -> Option<String> {
        let s = self.editor.project.scenes.get(scene)?;
        s.item(id).map(|i| match i.kind {
            ItemKind::Character(_) => "character".into(),
            ItemKind::Prop(_) => "prop".into(),
            ItemKind::Text(_) => "text".into(),
        })
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
