//! Desktop shell. The editor UI (shared with the web build) runs the engine
//! as WebAssembly for interactive editing; this native side adds what a
//! browser can't do well: real files and fast multi-threaded export via FFmpeg.

use frameling_core::{Library, Project};
use frameling_export::{export_video as run_export, ExportOptions};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

static CANCEL: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
fn write_text_file(path: String, contents: String) -> Result<(), String> {
    // Write to a temp file first so a crash never leaves a half-written project.
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, contents).map_err(|e| format!("{path}: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
fn cancel_export() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command]
async fn export_video(app: tauri::AppHandle, project: String, path: String, width: u32, height: u32) -> Result<(), String> {
    let project = Project::from_json(&project)?;
    CANCEL.store(false, Ordering::SeqCst);
    let mut opts = ExportOptions::for_project(&project);
    opts.width = width;
    opts.height = height;
    // Prefer an FFmpeg bundled in the app's resources, if present.
    if let Ok(res) = app.path().resource_dir() {
        let bundled = res.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
        if bundled.exists() {
            opts.ffmpeg = bundled;
        }
    }
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = Library::builtin();
        run_export(&project, lib, std::path::Path::new(&path), &opts, |done, total| {
            let _ = handle.emit("export-progress", (done, total));
            !CANCEL.load(Ordering::SeqCst)
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Speech engine: the bundled Supertonic model (app resources), then the OS voices.
fn tts(app: &tauri::AppHandle) -> frameling_tts::Tts {
    let mut bases = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        bases.push(res);
    }
    // `npm run tauri dev`: resources are not copied, use the source folder.
    bases.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"));
    bases
        .iter()
        .map(|b| frameling_tts::Tts::with_bundle(b))
        .find(|t| t.model_dir.is_some())
        .unwrap_or_else(|| frameling_tts::Tts::with_bundle(&bases[0]))
}

#[tauri::command]
async fn tts_voices(app: tauri::AppHandle) -> Result<Vec<frameling_tts::Voice>, String> {
    let t = tts(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let mut v = t.voices();
        // Same model may be found in two folders; keep the first.
        let mut seen = std::collections::HashSet::new();
        v.retain(|x| seen.insert(x.id.clone()));
        v
    })
    .await
    .map_err(|e| e.to_string())
}

/// Returns the spoken line as base64 WAV.
#[tauri::command]
async fn tts_synthesize(app: tauri::AppHandle, text: String, voice: String, speed: f32) -> Result<String, String> {
    use base64::Engine as _;
    let t = tts(&app);
    let wav = tauri::async_runtime::spawn_blocking(move || t.synthesize(&text, &voice, speed)).await.map_err(|e| e.to_string())??;
    Ok(base64::engine::general_purpose::STANDARD.encode(wav))
}

/// Licence terms of the built-in voices, shown once for the user to accept,
/// plus the path of the full licence text.
#[tauri::command]
fn tts_terms(app: tauri::AppHandle) -> (String, Option<String>) {
    let licence = tts(&app).model_dir.map(|d| d.join("LICENSE")).filter(|p| p.is_file()).map(|p| p.to_string_lossy().to_string());
    (frameling_tts::SUPERTONIC_TERMS.to_string(), licence)
}

/// Modification time (ms) of a file, so the editor can pick up edits made by
/// agents or scripts (the CLI / MCP server) to the open project.
#[tauri::command]
fn file_mtime(path: String) -> Result<u64, String> {
    let m = std::fs::metadata(&path).and_then(|m| m.modified()).map_err(|e| e.to_string())?;
    Ok(m.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_text_file,
            write_text_file,
            export_video,
            cancel_export,
            tts_voices,
            tts_synthesize,
            tts_terms,
            file_mtime
        ])
        .run(tauri::generate_context!())
        .expect("error while running Frameling");
}
