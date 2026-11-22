//! Minimal WAV reading for the native side (CLI and desktop TTS): duration and
//! the per-frame loudness envelope used for lip sync. The editor computes the
//! same envelope in the browser for other formats (MP3, OGG, recordings).

/// Decode a PCM WAV (8/16/24/32-bit int or 32-bit float) to mono samples.
pub fn wav_mono(bytes: &[u8]) -> Result<(Vec<f32>, u32), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let u16le = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32le = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let (mut fmt, mut channels, mut rate, mut bits) = (0u16, 0u16, 0u32, 0u16);
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        // Some encoders write 0xFFFFFFFF for streamed data chunks; clamp to the file.
        let size = (u32le(i + 4) as usize).min(bytes.len() - i - 8);
        let body = i + 8;
        if id == b"fmt " && size >= 16 {
            fmt = u16le(body);
            channels = u16le(body + 2).max(1);
            rate = u32le(body + 4);
            bits = u16le(body + 14);
            if fmt == 0xFFFE && size >= 26 {
                fmt = u16le(body + 24); // WAVE_FORMAT_EXTENSIBLE sub-format
            }
        } else if id == b"data" {
            let data = &bytes[body..body + size];
            let bps = (bits / 8) as usize;
            if rate == 0 || bps == 0 {
                return Err("WAV has no format chunk".into());
            }
            let frame = bps * channels as usize;
            let mut out = Vec::with_capacity(data.len() / frame.max(1));
            for f in data.chunks_exact(frame) {
                let mut acc = 0.0f32;
                for c in f.chunks_exact(bps) {
                    acc += match (fmt, bits) {
                        (1, 8) => (c[0] as f32 - 128.0) / 128.0,
                        (1, 16) => i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0,
                        (1, 24) => (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0,
                        (1, 32) => i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0,
                        (3, 32) => f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                        _ => return Err(format!("unsupported WAV encoding (format {fmt}, {bits} bit)")),
                    };
                }
                out.push(acc / channels as f32);
            }
            return Ok((out, rate));
        }
        i = body + size + (size & 1);
    }
    Err("WAV has no data chunk".into())
}

/// Loudness per video frame, normalised to 0..1 against the 95th percentile.
/// Must match `envelope()` in app/src/lib/audio.ts.
pub fn envelope(samples: &[f32], rate: u32, fps: u32) -> Vec<f32> {
    let per = (rate / fps.max(1)).max(1) as usize;
    let env: Vec<f32> = samples
        .chunks(per)
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len().max(1) as f32).sqrt())
        .collect();
    let mut sorted = env.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let r = sorted.get(sorted.len() * 95 / 100).copied().filter(|v| *v > 1e-6).unwrap_or(1e-6);
    env.iter().map(|v| ((v / r).min(1.0) * 1000.0).round() / 1000.0).collect()
}

/// Duration in frames and envelope for a WAV file.
pub fn analyse_wav(bytes: &[u8], fps: u32) -> Result<(u32, Vec<f32>), String> {
    let (s, rate) = wav_mono(bytes)?;
    let frames = ((s.len() as f64 / rate as f64) * fps as f64).ceil() as u32;
    Ok((frames, envelope(&s, rate, fps)))
}
