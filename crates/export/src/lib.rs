//! Native video export.
//!
//! Frames are evaluated and rendered one by one from integer frame numbers
//! (on several threads), then streamed in order to FFmpeg as raw RGBA. Audio
//! is placed by frame number too, so the result never drifts or drops frames,
//! however slow the machine is.

use base64::Engine as _;
use frameling_core::{audio_events, evaluate, Library, Project};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// Output size in pixels (defaults to the project size).
    pub width: u32,
    pub height: u32,
    /// FFmpeg executable (a bundled sidecar or one on PATH).
    pub ffmpeg: PathBuf,
    /// H.264 quality, 0 (lossless) to 51. 18-23 is a good range.
    pub crf: u8,
    /// Render threads (0 = number of CPUs).
    pub threads: usize,
}

impl ExportOptions {
    pub fn for_project(p: &Project) -> ExportOptions {
        ExportOptions { width: p.width, height: p.height, ffmpeg: find_ffmpeg(), crf: 20, threads: 0 }
    }
}

/// Look for FFmpeg next to the running executable first (bundled sidecar), then on PATH.
pub fn find_ffmpeg() -> PathBuf {
    let exe_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for candidate in [dir.join(exe_name), dir.join("../Resources").join(exe_name)] {
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from(exe_name)
}

fn ext_for(mime: &str, name: &str) -> String {
    if let Some(e) = Path::new(name).extension().and_then(|e| e.to_str()) {
        return e.to_ascii_lowercase();
    }
    match mime {
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        "audio/mp4" | "audio/aac" => "m4a",
        _ => "bin",
    }
    .into()
}

/// Export the whole project to a video file (MP4/H.264 + AAC, or by extension
/// `.webm` VP9 + Opus). `progress(done, total)` returns `false` to cancel.
pub fn export_video(
    project: &Project,
    lib: &Library,
    out: &Path,
    opts: &ExportOptions,
    mut progress: impl FnMut(u32, u32) -> bool,
) -> Result<(), String> {
    let total = project.total_frames();
    if total == 0 {
        return Err("nothing to export".into());
    }
    let (w, h) = (opts.width.max(2) & !1, opts.height.max(2) & !1);
    let fps = project.fps.max(1);

    // Write embedded media to a temp folder for FFmpeg.
    let tmp = std::env::temp_dir().join(format!("frameling-export-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("temp dir: {e}"))?;
    let _cleanup = Cleanup(tmp.clone());
    let events = audio_events(project);
    let mut media_files = std::collections::BTreeMap::new();
    for ev in &events {
        if media_files.contains_key(&ev.media) {
            continue;
        }
        let m = &project.media[&ev.media];
        let bytes = base64::engine::general_purpose::STANDARD.decode(m.data.as_bytes()).map_err(|e| format!("media {}: {e}", m.name))?;
        let path = tmp.join(format!("m{}.{}", media_files.len(), ext_for(&m.mime, &m.name)));
        std::fs::write(&path, bytes).map_err(|e| format!("write media: {e}"))?;
        media_files.insert(ev.media.clone(), path);
    }

    let webm = out.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("webm"));
    let mut cmd = Command::new(&opts.ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"]);
    for ev in &events {
        cmd.arg("-i").arg(&media_files[&ev.media]);
    }
    let secs = |frames: u32| frames as f64 / fps as f64;
    if !events.is_empty() {
        let mut filter = String::new();
        for (i, ev) in events.iter().enumerate() {
            let delay_ms = (secs(ev.at) * 1000.0).round() as u64;
            filter.push_str(&format!(
                "[{}:a]atrim=start={:.4}:duration={:.4},asetpts=PTS-STARTPTS,aresample=48000,volume={:.3},adelay={delay_ms}:all=1[a{i}];",
                i + 1,
                secs(ev.trim),
                secs(ev.frames),
                ev.volume
            ));
        }
        for i in 0..events.len() {
            filter.push_str(&format!("[a{i}]"));
        }
        filter.push_str(&format!("amix=inputs={}:normalize=0:duration=longest[aout]", events.len()));
        cmd.args(["-filter_complex", &filter, "-map", "0:v", "-map", "[aout]"]);
    }
    if webm {
        cmd.args(["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", &(opts.crf as u32 + 12).to_string(), "-row-mt", "1"]);
        if !events.is_empty() {
            cmd.args(["-c:a", "libopus", "-b:a", "160k"]);
        }
    } else {
        cmd.args(["-c:v", "libx264", "-preset", "medium", "-crf", &opts.crf.to_string(), "-pix_fmt", "yuv420p", "-movflags", "+faststart"]);
        if !events.is_empty() {
            cmd.args(["-c:a", "aac", "-b:a", "192k"]);
        }
    }
    cmd.args(["-t", &format!("{:.4}", secs(total))]).arg(out);
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("could not start FFmpeg ({}): {e}", opts.ffmpeg.display()))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    let threads = if opts.threads == 0 { std::thread::available_parallelism().map_or(4, |n| n.get()) } else { opts.threads };
    let batch = (threads * 2) as u32;
    let mut frame = 0;
    let mut cancelled = false;
    let mut write_err = None;
    while frame < total {
        let end = (frame + batch).min(total);
        let frames: Vec<Vec<u8>> = std::thread::scope(|s| {
            let handles: Vec<_> = (frame..end)
                .map(|f| s.spawn(move || frameling_render::render(&evaluate(project, lib, f), w, h)))
                .collect();
            handles.into_iter().map(|h| h.join().expect("render thread")).collect()
        });
        for px in frames {
            if let Err(e) = stdin.write_all(&px) {
                write_err = Some(e);
                break;
            }
        }
        if write_err.is_some() {
            break;
        }
        frame = end;
        if !progress(frame, total) {
            cancelled = true;
            break;
        }
    }
    drop(stdin);
    if cancelled {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(out);
        return Err("export cancelled".into());
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let log = err_reader.join().unwrap_or_default();
    if !status.success() || write_err.is_some() {
        let tail: Vec<&str> = log.lines().rev().take(6).collect();
        return Err(format!("FFmpeg failed: {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    Ok(())
}

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
