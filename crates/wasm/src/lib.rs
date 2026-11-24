//! The engine as seen from JavaScript.
//!
//! The editor UI keeps no document state of its own: it sends commands here,
//! reads the project back as JSON, and asks for rendered frames. The desktop
//! app and the web app use this exact same module.

use frameling_core::{
    audio_events, demo, eval, evaluate, Command, Editor, Item, ItemKind, Library, Project, Scene,
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

    /// Templates (id, name, description, best shape) as JSON.
    pub fn templates(&self) -> String {
        serde_json::to_string(&frameling_core::templates::catalog()).unwrap_or_default()
    }

    /// Replace the document with a template laid out for `shape` (vertical, wide, square).
    pub fn load_template(&mut self, id: &str, shape: &str) -> Result<(), JsError> {
        let p = frameling_core::templates::build(id, shape).ok_or_else(|| JsError::new(&format!("no template {id:?}")))?;
        self.editor.load(p);
        Ok(())
    }

    /// A template's first scene, a little way in, as RGBA pixels (the picker's thumbnails).
    pub fn template_thumb(&self, id: &str, shape: &str, width: u32, height: u32) -> Vec<u8> {
        let Some(p) = frameling_core::templates::build_with(id, shape, false) else { return vec![] };
        let f = (p.fps * 2).min(p.scenes.first().map_or(1, |s| s.duration.saturating_sub(1)));
        frameling_render::render(&frameling_core::evaluate(&p, &self.lib, f), width, height)
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

    /// Built-in sound effects and music loops (id, name, kind, category, secs) as JSON.
    pub fn sound_catalog(&self) -> String {
        serde_json::to_string(&frameling_core::sfx::catalog()).unwrap_or_default()
    }

    /// A built-in sound as WAV bytes (previews).
    pub fn sound_wav(&self, id: &str) -> Option<Vec<u8>> {
        frameling_core::sfx::render(id)
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

    /// `[x, y, zoom]` of a scene's camera at a local frame (scene move plus
    /// camera keyframes), so the stage overlay maps coordinates like the renderer.
    pub fn camera_at(&self, scene: usize, local: u32) -> Vec<f32> {
        match self.editor.project.scenes.get(scene) {
            Some(s) => {
                let k = eval::camera_at(s, local);
                vec![k.x, k.y, k.zoom]
            }
            None => vec![640.0, 360.0, 1.0],
        }
    }

    pub fn last_hits(&self) -> String {
        self.last_hits.clone()
    }

    /// Render a global frame (playback and export), with transitions.
    pub fn render_frame(&self, frame: u32, width: u32, height: u32) -> Vec<u8> {
        frameling_render::render(&evaluate(&self.editor.project, &self.lib, frame), width, height)
    }

    /// The movie's auto subtitles as an SRT file.
    pub fn captions_srt(&self) -> String {
        frameling_core::captions::srt(&self.editor.project)
    }

    /// Caption settings suited to the project's frame shape, as JSON.
    pub fn caption_defaults(&self) -> String {
        let p = &self.editor.project;
        serde_json::to_string(&frameling_core::captions::defaults_for(p.width, p.height)).unwrap_or_default()
    }

    /// Audio placements for playback/export, as JSON.
    pub fn audio_events(&self) -> String {
        serde_json::to_string(&audio_events(&self.editor.project)).unwrap_or_default()
    }

    /// Render a single item (JSON) centred on a transparent canvas: library
    /// thumbnails and drag previews.
    pub fn render_item_preview(&self, item_json: &str, width: u32, height: u32, frame: u32) -> Result<Vec<u8>, JsError> {
        let item: Item = serde_json::from_str(item_json).map_err(err)?;
        let dl = eval::item_preview(&self.lib, &self.editor.project.parts, &item, width, height, frame);
        Ok(unpremultiply(frameling_render::render(&dl, width, height)))
    }

    /// Parts catalogue (built-in and pack parts) as JSON; `style` filters.
    pub fn parts_catalog(&self, style: Option<String>) -> String {
        serde_json::to_string(&frameling_core::parts::catalog(&self.lib, style.as_deref())).unwrap_or_default()
    }

    /// Put a part on a look (JSON in, JSON out).
    pub fn apply_part(&self, look_json: &str, part: &str, colors_json: &str) -> Result<String, JsError> {
        let mut look: frameling_core::Look = serde_json::from_str(look_json).map_err(err)?;
        let colors: Vec<frameling_core::Color> = serde_json::from_str(colors_json).unwrap_or_default();
        frameling_core::parts::apply(&mut look, &self.lib, Some(&self.editor.project), part, &colors).map_err(err)?;
        Ok(serde_json::to_string(&look).unwrap())
    }

    /// Which part a look uses per category, as JSON {category: partId}.
    pub fn selected_parts(&self, look_json: &str) -> Result<String, JsError> {
        let look: frameling_core::Look = serde_json::from_str(look_json).map_err(err)?;
        Ok(serde_json::to_string(&frameling_core::parts::selected(&look, &self.lib, Some(&self.editor.project))).unwrap())
    }

    /// A random look in the same style; `locks` is a JSON list of categories to keep.
    pub fn randomize_look(&self, look_json: &str, seed: f64, locks_json: &str) -> Result<String, JsError> {
        let look: frameling_core::Look = serde_json::from_str(look_json).map_err(err)?;
        let locks: Vec<String> = serde_json::from_str(locks_json).unwrap_or_default();
        let out = frameling_core::parts::randomize(&look, &self.lib, Some(&self.editor.project), seed as u64, &locks);
        Ok(serde_json::to_string(&out).unwrap())
    }

    /// A character's full pose at a frame (JSON), to save it as a pose.
    pub fn character_pose(&self, scene: usize, id: &str, local: u32) -> Option<String> {
        let sc = self.editor.project.scenes.get(scene)?;
        eval::posed_character(&self.editor.project, &self.lib, &sc.id, id, local).map(|p| serde_json::to_string(&p).unwrap())
    }

    /// Bone track values that hold a pose (JSON [[path, value], ...]), for keying it.
    pub fn pose_keys(&self, pose: &str, mask: &str, mirror: bool) -> Result<String, JsError> {
        use frameling_core::poses;
        let def = poses::find(Some(&self.editor.project), &self.lib, pose).ok_or_else(|| err(format!("unknown pose {pose:?}")))?;
        let p = if mirror { poses::mirror(&def.pose) } else { def.pose };
        let mask: poses::PoseMask = serde_json::from_value(serde_json::Value::String(mask.into())).map_err(err)?;
        Ok(serde_json::to_string(&poses::pose_keys(&p, mask)).unwrap())
    }

    /// Chain poses into an animation (JSON ActionDef) from [{pose, hold, mirror}].
    pub fn chain_poses(&self, id: &str, name: &str, steps_json: &str, gap: f32, looped: bool) -> Result<String, JsError> {
        let steps: Vec<frameling_core::poses::ChainStep> = serde_json::from_str(steps_json).map_err(err)?;
        let a = frameling_core::poses::chain(id, name, &steps, gap, looped, Some(&self.editor.project), &self.lib).map_err(err)?;
        Ok(serde_json::to_string(&a).unwrap())
    }

    /// Definitions of pack parts that are not built in (to copy into the
    /// project with the addParts command). `ids_json` is a list of part ids.
    pub fn part_defs(&self, ids_json: &str) -> String {
        let ids: Vec<String> = serde_json::from_str(ids_json).unwrap_or_default();
        let builtin = &frameling_core::Library::builtin().parts;
        let defs: Vec<_> = ids.iter().filter(|i| !builtin.contains_key(*i)).filter_map(|i| self.lib.parts.get(i).map(|p| p.def.clone())).collect();
        serde_json::to_string(&defs).unwrap()
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

    /// An item (JSON) as it looks at a scene-local frame, with keyed values
    /// baked into its plain fields: what the inspector shows and edits.
    pub fn posed_item(&self, item_json: &str, frame: u32) -> Result<String, JsError> {
        let item: frameling_core::model::Item = serde_json::from_str(item_json).map_err(err)?;
        serde_json::to_string(&frameling_core::anim::posed_item(&item, frame)).map_err(err)
    }

    /// The kind of an item as a string, handy for the UI.
    pub fn item_kind(&self, scene: usize, id: &str) -> Option<String> {
        let s = self.editor.project.scenes.get(scene)?;
        s.item(id).map(|i| match i.kind {
            ItemKind::Character(_) => "character".into(),
            ItemKind::Prop(_) => "prop".into(),
            ItemKind::Text(_) => "text".into(),
            ItemKind::Image(_) => "image".into(),
        })
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
