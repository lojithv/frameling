//! Write a line to a WAV file, for auditioning voices.
//!
//!   cargo run --release -p frameling-tts --example speak -- <resources-dir> <voice-id> "text" out.wav [speed]
//!
//! `<resources-dir>` contains `tts/supertonic` (e.g. app/src-tauri/resources).

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        return Err("usage: speak <resources-dir> <voice-id> <text> <out.wav> [speed]".into());
    }
    let tts = frameling_tts::Tts::with_bundle(std::path::Path::new(&a[0]));
    let speed = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let t = std::time::Instant::now();
    let wav = tts.synthesize(&a[2], &a[1], speed)?;
    std::fs::write(&a[3], &wav).map_err(|e| e.to_string())?;
    let secs = (wav.len() - 44) as f32 / 2.0 / 44100.0;
    println!("{}: {secs:.2}s of audio in {:.2}s", a[3], t.elapsed().as_secs_f32());
    Ok(())
}
