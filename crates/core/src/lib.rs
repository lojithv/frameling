//! Frameling core: the document model, commands with undo, asset packs, the
//! character rig and the deterministic timeline evaluator.
//!
//! This crate has no platform dependencies. It is compiled natively for the
//! desktop app and CLI, and to WebAssembly for the browser editor.

pub mod anim;
pub mod audio;
pub mod camera;
pub mod captions;
pub mod command;
pub mod demo;
pub mod display;
pub mod eval;
pub mod geom;
pub mod lipsync;
pub mod model;
pub mod mouths;
pub mod pack;
pub mod parts;
pub mod poses;
pub mod rig;
pub mod sfx;
pub mod stick;
pub mod templates;
pub mod text;

pub use anim::{Interp, Key, Track, Tracks};
pub use command::{Command, Editor};
pub use display::{DisplayList, Op, Paint, Path, Seg};
pub use eval::{audio_events, evaluate, evaluate_scene, AudioEvent, ItemHit};
pub use geom::{Affine, Color, Ease, Rect};
pub use model::*;
pub use pack::Library;
