//! Commands: the only way the document changes.
//!
//! The UI, plugins, the CLI and scripts all send the same serialisable
//! commands, so anything a person can do can also be automated, and every
//! change is undoable.

use crate::model::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    SetMeta { name: Option<String>, fps: Option<u32>, width: Option<u32>, height: Option<u32> },
    AddScene { scene: Scene, index: Option<usize> },
    RemoveScene { id: Id },
    MoveScene { id: Id, to: usize },
    /// Replace a scene's settings. Items and audio are left untouched.
    UpdateScene { scene: Scene },
    AddItem { scene: Id, item: Item, index: Option<usize> },
    RemoveItem { scene: Id, id: Id },
    UpdateItem { scene: Id, item: Item },
    /// Change draw order (0 = back).
    MoveItem { scene: Id, id: Id, to: usize },
    AddMedia { media: Media },
    RemoveMedia { id: Id },
    AddAudio { scene: Id, clip: AudioClip },
    UpdateAudio { scene: Id, clip: AudioClip },
    RemoveAudio { scene: Id, id: Id },
    /// Add a spoken line: imports the media, places it on the scene (after the
    /// previous line unless `start` is given) and lengthens the scene to fit,
    /// like a dialogue track.
    AddLine { scene: Id, clip_id: Id, media: Media, speaker: Option<Id>, start: Option<u32> },
    /// Several commands applied as one undo step.
    Batch { commands: Vec<Command> },
}

fn scene_mut<'a>(p: &'a mut Project, id: &str) -> Result<&'a mut Scene, String> {
    p.scenes.iter_mut().find(|s| s.id == id).ok_or_else(|| format!("no scene {id:?}"))
}

fn check_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        Err("id must not be empty".into())
    } else {
        Ok(())
    }
}

impl Command {
    /// Apply to a project. On error the project may be partially changed, so
    /// callers apply to a copy (the editor does this for undo anyway).
    pub fn apply(&self, p: &mut Project) -> Result<(), String> {
        match self {
            Command::SetMeta { name, fps, width, height } => {
                if let Some(n) = name {
                    p.name = n.clone();
                }
                if let Some(f) = fps {
                    p.fps = (*f).clamp(1, 120);
                }
                if let Some(w) = width {
                    p.width = (*w).clamp(64, 7680) & !1;
                }
                if let Some(h) = height {
                    p.height = (*h).clamp(64, 4320) & !1;
                }
            }
            Command::AddScene { scene, index } => {
                check_id(&scene.id)?;
                if p.scene(&scene.id).is_some() {
                    return Err(format!("scene {:?} already exists", scene.id));
                }
                let i = index.unwrap_or(p.scenes.len()).min(p.scenes.len());
                p.scenes.insert(i, scene.clone());
            }
            Command::RemoveScene { id } => {
                if p.scenes.len() <= 1 {
                    return Err("a project needs at least one scene".into());
                }
                let i = p.scene_index(id).ok_or_else(|| format!("no scene {id:?}"))?;
                p.scenes.remove(i);
            }
            Command::MoveScene { id, to } => {
                let i = p.scene_index(id).ok_or_else(|| format!("no scene {id:?}"))?;
                let s = p.scenes.remove(i);
                let to = (*to).min(p.scenes.len());
                p.scenes.insert(to, s);
            }
            Command::UpdateScene { scene } => {
                let s = scene_mut(p, &scene.id)?;
                let items = std::mem::take(&mut s.items);
                let audio = std::mem::take(&mut s.audio);
                *s = Scene { items, audio, duration: scene.duration.max(1), ..scene.clone() };
            }
            Command::AddItem { scene, item, index } => {
                check_id(&item.id)?;
                let s = scene_mut(p, scene)?;
                if s.item(&item.id).is_some() {
                    return Err(format!("item {:?} already exists", item.id));
                }
                let i = index.unwrap_or(s.items.len()).min(s.items.len());
                s.items.insert(i, item.clone());
            }
            Command::RemoveItem { scene, id } => {
                let s = scene_mut(p, scene)?;
                let before = s.items.len();
                s.items.retain(|i| &i.id != id);
                if s.items.len() == before {
                    return Err(format!("no item {id:?}"));
                }
                // Speech by a removed character stays as plain audio.
                for c in &mut s.audio {
                    if c.speaker.as_deref() == Some(id.as_str()) {
                        c.speaker = None;
                    }
                }
            }
            Command::UpdateItem { scene, item } => {
                let s = scene_mut(p, scene)?;
                let slot = s.items.iter_mut().find(|i| i.id == item.id).ok_or_else(|| format!("no item {:?}", item.id))?;
                *slot = item.clone();
            }
            Command::MoveItem { scene, id, to } => {
                let s = scene_mut(p, scene)?;
                let i = s.items.iter().position(|i| &i.id == id).ok_or_else(|| format!("no item {id:?}"))?;
                let it = s.items.remove(i);
                let to = (*to).min(s.items.len());
                s.items.insert(to, it);
            }
            Command::AddMedia { media } => {
                check_id(&media.id)?;
                p.media.insert(media.id.clone(), Arc::new(media.clone()));
            }
            Command::RemoveMedia { id } => {
                let used = p.scenes.iter().any(|s| s.audio.iter().any(|c| &c.media == id));
                if used {
                    return Err("this sound is still used in a scene".into());
                }
                p.media.remove(id);
            }
            Command::AddAudio { scene, clip } => {
                check_id(&clip.id)?;
                if !p.media.contains_key(&clip.media) {
                    return Err(format!("no media {:?}", clip.media));
                }
                let s = scene_mut(p, scene)?;
                if s.audio.iter().any(|c| c.id == clip.id) {
                    return Err(format!("audio clip {:?} already exists", clip.id));
                }
                s.audio.push(clip.clone());
            }
            Command::UpdateAudio { scene, clip } => {
                let s = scene_mut(p, scene)?;
                let slot = s.audio.iter_mut().find(|c| c.id == clip.id).ok_or_else(|| format!("no audio clip {:?}", clip.id))?;
                *slot = clip.clone();
            }
            Command::RemoveAudio { scene, id } => {
                let s = scene_mut(p, scene)?;
                s.audio.retain(|c| &c.id != id);
            }
            Command::AddLine { scene, clip_id, media, speaker, start } => {
                check_id(clip_id)?;
                check_id(&media.id)?;
                let fps = p.fps.max(1);
                let idx = p.scene_index(scene).ok_or_else(|| format!("no scene {scene:?}"))?;
                if let Some(sp) = speaker {
                    if p.scenes[idx].item(sp).is_none() {
                        return Err(format!("no character {sp:?} in this scene"));
                    }
                }
                // Default: right after the last spoken line, like a dialogue track.
                let at = start.unwrap_or_else(|| {
                    let sc = &p.scenes[idx];
                    sc.audio
                        .iter()
                        .filter(|c| c.speaker.is_some())
                        .filter_map(|c| p.media.get(&c.media).map(|m| c.start + c.play_frames(m)))
                        .max()
                        .map_or(fps / 3, |e| e + fps / 5)
                });
                p.media.insert(media.id.clone(), Arc::new(media.clone()));
                let s = &mut p.scenes[idx];
                let end = at + media.duration_frames;
                if end + fps / 2 > s.duration {
                    s.duration = end + fps / 2;
                }
                s.audio.push(AudioClip { id: clip_id.clone(), media: media.id.clone(), start: at, speaker: speaker.clone(), ..Default::default() });
            }
            Command::Batch { commands } => {
                for c in commands {
                    c.apply(p)?;
                }
            }
        }
        Ok(())
    }
}

/// Holds the project plus undo/redo history.
///
/// Undo is snapshot based. Media is behind `Arc`, and scenes are small, so a
/// snapshot is cheap; this keeps every command trivially undoable.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    pub project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// Consecutive commands with the same merge key (e.g. one drag gesture)
    /// collapse into a single undo step.
    last_merge: Option<String>,
    pub dirty: bool,
}

const MAX_UNDO: usize = 200;

impl Editor {
    pub fn new(project: Project) -> Editor {
        Editor { project, ..Default::default() }
    }

    pub fn apply(&mut self, cmd: &Command, merge: Option<&str>) -> Result<(), String> {
        let mut next = self.project.clone();
        cmd.apply(&mut next)?;
        let merging = merge.is_some() && merge == self.last_merge.as_deref();
        if !merging {
            self.undo.push(std::mem::replace(&mut self.project, next));
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
        } else {
            self.project = next;
        }
        self.last_merge = merge.map(str::to_string);
        self.redo.clear();
        self.dirty = true;
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        self.last_merge = None;
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.project, prev));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        self.last_merge = None;
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.project, next));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Ends the current merge group (call when a drag gesture ends).
    pub fn seal(&mut self) {
        self.last_merge = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn load(&mut self, project: Project) {
        *self = Editor::new(project);
    }
}
