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
    /// Render every n-th frame (2 = half the frame rate, for quick drafts).
    /// Timing and audio are unchanged; motion is just less smooth.
    pub step: u32,
}

impl ExportOptions {
    pub fn for_project(p: &Project) -> ExportOptions {
        ExportOptions { width: p.width, height: p.height, ffmpeg: find_ffmpeg(), crf: 20, threads: 0, step: 1 }
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
    // An app started from Finder or the Dock gets a minimal PATH without
    // Homebrew, so look in the usual install places before trusting PATH.
    if !cfg!(windows) && !on_path(exe_name) {
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin", "/snap/bin"] {
            let candidate = Path::new(dir).join(exe_name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(exe_name)
}

/// Whether a program can be found on PATH.
fn on_path(exe_name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(exe_name).is_file()))
}

/// File extension for a media file handed to FFmpeg. The MIME type wins: a
/// media *name* can be a line of dialogue ("Almost. The voices sound…"), and
/// its "extension" is not a file type.
fn ext_for(mime: &str, name: &str) -> String {
    let from_mime = match mime {
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        "audio/mp4" | "audio/aac" => "m4a",
        _ => "",
    };
    if !from_mime.is_empty() {
        return from_mime.into();
    }
    match Path::new(name).extension().and_then(|e| e.to_str()) {
        Some(e) if (1..=5).contains(&e.len()) && e.chars().all(|c| c.is_ascii_alphanumeric()) => e.to_ascii_lowercase(),
        _ => "bin".into(),
    }
}

#[cfg(test)]
mod ext_tests {
    use super::ext_for;

    #[test]
    fn dialogue_names_do_not_become_extensions() {
        assert_eq!(ext_for("audio/wav", "Almost. The voices sound…"), "wav");
        assert_eq!(ext_for("", "Almost. The voices sound…"), "bin");
        assert_eq!(ext_for("", "take 3.FLAC"), "flac");
    }
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
    let step = opts.step.clamp(1, fps);

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
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &format!("{fps}/{step}"), "-i", "-"]);
    for ev in &events {
        if ev.looped {
            cmd.args(["-stream_loop", "-1"]);
        }
        cmd.arg("-i").arg(&media_files[&ev.media]);
    }
    let secs = |frames: u32| frames as f64 / fps as f64;
    if !events.is_empty() {
        let mut filter = String::new();
        for (i, ev) in events.iter().enumerate() {
            let delay_ms = (secs(ev.at) * 1000.0).round() as u64;
            let shaped = ev.fade_in > 0 || ev.fade_out > 0 || !ev.ducks.is_empty();
            let (pre, post) = if shaped {
                (String::new(), format!(",volume='{}':eval=frame", gain_expr(ev, fps)))
            } else {
                (format!(",volume={:.3}", ev.volume), String::new())
            };
            filter.push_str(&format!(
                "[{}:a]atrim=start={:.4}:duration={:.4},asetpts=PTS-STARTPTS,aresample=48000{pre},adelay={delay_ms}:all=1{post}[a{i}];",
                i + 1,
                secs(ev.trim),
                secs(ev.frames),
            ));
        }
        for i in 0..events.len() {
            filter.push_str(&format!("[a{i}]"));
        }
        // Stereo out: FFmpeg 9.1's AAC encoder can loop forever on a 192 kb/s mono
        // stream (all voice lines are mono), which froze exports on Windows.
        filter.push_str(&format!("amix=inputs={}:normalize=0:duration=longest,aformat=channel_layouts=stereo[aout]", events.len()));
        // Long ducking expressions can pass Windows' command-line limit, so
        // the graph goes in a file.
        let graph = tmp.join("graph.txt");
        std::fs::write(&graph, &filter).map_err(|e| format!("write filter graph: {e}"))?;
        cmd.arg("-/filter_complex").arg(&graph).args(["-map", "0:v", "-map", "[aout]"]);
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
    let picks: Vec<u32> = (0..total).step_by(step as usize).collect();
    let mut next = 0;
    let mut cancelled = false;
    let mut write_err = None;
    while next < picks.len() {
        let end = (next + batch as usize).min(picks.len());
        let frames: Vec<Vec<u8>> = std::thread::scope(|s| {
            let handles: Vec<_> = picks[next..end]
                .iter()
                .map(|&f| s.spawn(move || frameling_render::render(&evaluate(project, lib, f), w, h)))
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
        next = end;
        let done = picks.get(next).copied().unwrap_or(total);
        if !progress(done, total) {
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

/// The event's gain curve (`AudioEvent::gain`) as an FFmpeg expression of
/// `t`, the output time in seconds (the filter runs after `adelay`).
fn gain_expr(ev: &frameling_core::AudioEvent, fps: u32) -> String {
    let s = |f: f64| f / fps as f64;
    let start = s(ev.at as f64);
    let end = s((ev.at + ev.frames) as f64);
    let mut e = format!("{:.4}", ev.volume);
    if ev.fade_in > 0 {
        e.push_str(&format!("*clip((t-{start:.4})/{:.4},0,1)", s(ev.fade_in as f64)));
    }
    if ev.fade_out > 0 {
        e.push_str(&format!("*clip(({end:.4}-t)/{:.4},0,1)", s(ev.fade_out as f64)));
    }
    if !ev.ducks.is_empty() {
        let r = s(ev.duck_ramp.max(1) as f64);
        let terms: Vec<String> = ev
            .ducks
            .iter()
            .map(|&[a, b]| format!("clip((t-{:.4})/{r:.4},0,1)*clip(({:.4}-t)/{r:.4},0,1)", s(a as f64) - r, s(b as f64) + r))
            .collect();
        e.push_str(&format!("*(1-{:.4}*min(1,{}))", 1.0 - ev.duck_level, terms.join("+")));
    }
    e
}

#[cfg(test)]
mod gain_tests {
    #[test]
    fn expression_has_every_part() {
        let ev = frameling_core::AudioEvent { media: "m".into(), at: 30, frames: 300, volume: 0.5, fade_in: 15, fade_out: 30, ducks: vec![[60, 90], [150, 200]], duck_level: 0.3, duck_ramp: 7, ..Default::default() };
        let e = super::gain_expr(&ev, 30);
        assert!(e.starts_with("0.5000*clip((t-1.0000)/0.5000,0,1)*clip((11.0000-t)/1.0000,0,1)*(1-0.7000*min(1,"), "{e}");
        assert_eq!(e.matches("clip(").count(), 6);
    }
}

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
