//! Offline text-to-speech.
//!
//! * **Supertonic 3** (neural, bundled): Supertone's multilingual ONNX model,
//!   run in-process with ONNX Runtime. Ten voice styles, 31 languages. The
//!   weights ship in the app's resources (see `app/scripts/fetch-tts.mjs`) and
//!   are under the OpenRAIL-M licence, separate from Frameling's own.
//! * **System voices** (fallback, zero download): `say` on macOS, SAPI on
//!   Windows, eSpeak NG on Linux. These run as separate processes.
//!
//! Every engine returns a WAV file, which the app imports like any other sound,
//! so lip sync and export work the same for typed, recorded and uploaded lines.
//!
//! Voice ids stored in projects: `supertonic:F1` (English) or
//! `supertonic:F1@de` (any of the 31 languages), and `system:<name>`.
//! Older projects may name `piper:…` voices; those are re-voiced with a
//! Supertonic voice picked from the old id, so each character keeps a distinct voice.

pub mod supertonic;

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    /// Stable id stored in projects, e.g. `supertonic:F1`, `supertonic:M2@fr` or `system:Microsoft Zira Desktop`.
    pub id: String,
    pub name: String,
    /// BCP-47-ish language tag, e.g. `en-US`. Supertonic voices list `en`; append `@xx` to the id for other languages.
    pub language: String,
    /// "supertonic" or "system".
    pub engine: String,
    /// Short credit line shown in the UI (licence / source).
    pub credit: String,
}

/// Shown wherever the built-in voices are offered (UI, CLI, MCP).
pub const SUPERTONIC_CREDIT: &str = "Supertonic 3 by Supertone, OpenRAIL-M licence";

/// The part of the OpenRAIL-M licence users must agree to before using the
/// built-in voices (paragraph 5 and Attachment A, summarised; the full text is
/// `tts/supertonic/LICENSE`).
pub const SUPERTONIC_TERMS: &str = "The built-in voices use the Supertonic 3 model by Supertone Inc., licensed under the \
BigScience OpenRAIL-M licence. By using them you agree to its use restrictions (Attachment A), including: \
do not impersonate a real person or create deepfakes without their consent; say clearly that the speech is \
machine-generated wherever you publish it; do not use it to harm, harass, defame or discriminate against people, \
to break the law, to spread false information meant to harm others, or to give medical advice. You are \
responsible for what you make with it. Full licence: tts/supertonic/LICENSE in the app folder.";

#[derive(Clone, Debug)]
pub struct Tts {
    /// Folder holding the Supertonic model (`onnx/`, `voice_styles/`), if installed.
    pub model_dir: Option<PathBuf>,
    /// Denoising steps for Supertonic (quality vs speed).
    pub steps: usize,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(exe(name))).find(|p| p.is_file())
}

fn tmp_path(ext: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!("frameling-tts-{}-{}.{ext}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}

fn run(mut cmd: Command, stdin_text: Option<&str>) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    cmd.stdin(if stdin_text.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().map_err(|e| format!("could not start speech engine: {e}"))?;
    if let (Some(t), Some(mut si)) = (stdin_text, child.stdin.take()) {
        si.write_all(t.as_bytes()).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("speech engine failed: {}", err.lines().last().unwrap_or("unknown error")));
    }
    Ok(())
}

fn read_and_remove(p: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("no audio produced: {e}"))?;
    let _ = std::fs::remove_file(p);
    if bytes.len() < 64 {
        return Err("the speech engine produced no audio".into());
    }
    Ok(bytes)
}

/// 16-bit PCM mono WAV, peak-limited so loud lines never clip.
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let gain = if peak > 0.98 { 0.98 / peak } else { 1.0 };
    let data_len = (samples.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&sample_rate.to_le_bytes());
    w.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        w.extend_from_slice(&(((s * gain).clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    w
}

/// Supertonic pads each line with ~0.45 s of silence before and ~0.65 s after.
/// Trim it (keeping a short breath) so a line starts when the character starts
/// talking and back-to-back lines don't have a second of dead air between them.
pub fn trim_silence(s: &[f32], sample_rate: u32) -> &[f32] {
    let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if peak <= 0.0 {
        return s;
    }
    let th = peak * 0.02;
    let (Some(a), Some(b)) = (s.iter().position(|x| x.abs() > th), s.iter().rposition(|x| x.abs() > th)) else { return s };
    let sr = sample_rate as f32;
    let a = a.saturating_sub((0.06 * sr) as usize);
    let b = (b + (0.15 * sr) as usize).min(s.len() - 1);
    &s[a..=b]
}

/// Split `supertonic:F1@de` into (`F1`, `de`).
fn parse_supertonic(rest: &str) -> Result<(&str, &str), String> {
    let (voice, lang) = rest.split_once('@').unwrap_or((rest, "en"));
    let lang = lang.split(['-', '_']).next().unwrap_or("en");
    if !supertonic::VOICES.iter().any(|(id, _)| *id == voice) {
        return Err(format!("unknown voice supertonic:{rest}"));
    }
    Ok((voice, lang))
}

/// Pick a Supertonic voice for an old Piper id, stable per id.
fn legacy_piper_voice(rest: &str) -> String {
    let lang = rest.split(['-', '_']).next().unwrap_or("en");
    let lang = if supertonic::LANGS.contains(&lang) { lang } else { "en" };
    let h = rest.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let (id, _) = supertonic::VOICES[h as usize % supertonic::VOICES.len()];
    if lang == "en" { id.to_string() } else { format!("{id}@{lang}") }
}

impl Tts {
    /// Look for the bundled model next to the app (`<base>/tts/supertonic`).
    pub fn with_bundle(base: &Path) -> Tts {
        let dir = base.join("tts").join("supertonic");
        Tts { model_dir: supertonic::is_model_dir(&dir).then_some(dir), steps: supertonic::DEFAULT_STEPS }
    }

    /// All voices available on this machine. Built-in voices come first.
    pub fn voices(&self) -> Vec<Voice> {
        let mut out = Vec::new();
        if let Some(dir) = &self.model_dir {
            for (id, name) in supertonic::installed_voices(dir) {
                let kind = if id.starts_with('F') { "female" } else { "male" };
                out.push(Voice {
                    id: format!("supertonic:{id}"),
                    name: format!("{name} ({kind})"),
                    language: "en".into(),
                    engine: "supertonic".into(),
                    credit: SUPERTONIC_CREDIT.into(),
                });
            }
        }
        out.extend(system_voices());
        out
    }

    /// Speak `text` with `voice_id` at `speed` (1.0 = normal). Returns WAV bytes.
    pub fn synthesize(&self, text: &str, voice_id: &str, speed: f32) -> Result<Vec<u8>, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("nothing to say".into());
        }
        if text.len() > 5000 {
            return Err("that line is too long (5000 characters max)".into());
        }
        let speed = speed.clamp(0.5, 2.0);
        let (engine, rest) = voice_id.split_once(':').ok_or("unknown voice")?;
        match engine {
            "supertonic" => {
                let (voice, lang) = parse_supertonic(rest)?;
                let dir = self.model_dir.as_ref().ok_or("the built-in voices are not installed (run `npm run fetch-tts`)")?;
                let model = supertonic::shared(dir)?;
                let mut m = model.lock().map_err(|_| "speech engine lock poisoned")?;
                let samples = m.synthesize(text, lang, voice, speed, self.steps)?;
                let sr = m.sample_rate();
                Ok(encode_wav(trim_silence(&samples, sr), sr))
            }
            "piper" if self.model_dir.is_some() => self.synthesize(text, &format!("supertonic:{}", legacy_piper_voice(rest)), speed),
            "system" => system_say(text, rest, speed),
            _ => Err(format!("voice {voice_id:?} is not available on this machine")),
        }
    }
}

// ---------------------------------------------------------------- system voices

/// Voices provided by the operating system.
pub fn system_voices() -> Vec<Voice> {
    let mk = |id: String, name: String, language: String, credit: &str| Voice { id: format!("system:{id}"), name, language, engine: "system".into(), credit: credit.into() };
    let mut out = Vec::new();
    if cfg!(target_os = "macos") {
        if let Ok(o) = Command::new("say").args(["-v", "?"]).output() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                // "Samantha            en_US    # Hello! My name is Samantha."
                let head = line.split('#').next().unwrap_or("");
                let toks: Vec<&str> = head.split_whitespace().collect();
                if toks.len() >= 2 {
                    let lang = toks[toks.len() - 1].replace('_', "-");
                    let name = toks[..toks.len() - 1].join(" ");
                    out.push(mk(name.clone(), name, lang, "macOS voice"));
                }
            }
        }
    } else if cfg!(windows) {
        let script = "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.GetInstalledVoices() | ForEach-Object { $_.VoiceInfo.Name + '|' + $_.VoiceInfo.Culture.Name }";
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        if let Ok(o) = cmd.output() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if let Some((name, lang)) = line.trim().split_once('|') {
                    out.push(mk(name.to_string(), name.replace("Microsoft ", "").replace(" Desktop", ""), lang.to_string(), "Windows voice"));
                }
            }
        }
    } else if let Some(espeak) = which("espeak-ng").or_else(|| which("espeak")) {
        if let Ok(o) = Command::new(espeak).arg("--voices").output() {
            for line in String::from_utf8_lossy(&o.stdout).lines().skip(1) {
                let toks: Vec<&str> = line.split_whitespace().collect();
                if toks.len() >= 4 {
                    let lang = toks[1].to_string();
                    let name = toks[3].replace('_', " ");
                    out.push(mk(lang.clone(), format!("{name} (eSpeak)"), lang, "eSpeak NG"));
                }
            }
        }
    }
    out
}

fn system_say(text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, String> {
    let txt = tmp_path("txt");
    std::fs::write(&txt, text).map_err(|e| e.to_string())?;
    let out = tmp_path("wav");
    let result = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("say");
        cmd.arg("-v").arg(voice).arg("-r").arg(format!("{}", (180.0 * speed).round()))
            .arg("--file-format=WAVE").arg("--data-format=LEI16@22050").arg("-o").arg(&out).arg("-f").arg(&txt);
        run(cmd, None)
    } else if cfg!(windows) {
        // Arguments go through environment variables, never into the script text.
        let script = "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
            $s.SelectVoice($env:FL_VOICE); $s.Rate = [int]$env:FL_RATE; $s.SetOutputToWaveFile($env:FL_OUT); \
            $s.Speak([IO.File]::ReadAllText($env:FL_TEXT)); $s.Dispose()";
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("FL_VOICE", voice)
            .env("FL_RATE", format!("{}", ((speed - 1.0) * 10.0).round().clamp(-10.0, 10.0)))
            .env("FL_OUT", &out)
            .env("FL_TEXT", &txt);
        run(cmd, None)
    } else {
        let espeak = which("espeak-ng").or_else(|| which("espeak")).ok_or("no system speech engine found (install espeak-ng)")?;
        let mut cmd = Command::new(espeak);
        cmd.arg("-v").arg(voice).arg("-s").arg(format!("{}", (165.0 * speed).round())).arg("-w").arg(&out).arg("-f").arg(&txt);
        run(cmd, None)
    };
    let _ = std::fs::remove_file(&txt);
    result?;
    read_and_remove(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled() -> Option<Tts> {
        // FRAMELING_TTS points at a folder containing `tts/supertonic`.
        let base = std::env::var_os("FRAMELING_TTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/resources"));
        let t = Tts::with_bundle(&base);
        t.model_dir.is_some().then_some(t)
    }

    #[test]
    fn voice_ids_parse() {
        assert_eq!(parse_supertonic("F1").unwrap(), ("F1", "en"));
        assert_eq!(parse_supertonic("M3@pt-BR").unwrap(), ("M3", "pt"));
        assert!(parse_supertonic("../x").is_err());
        assert!(parse_supertonic("F9").is_err());
    }

    #[test]
    fn legacy_piper_ids_map_to_distinct_stable_voices() {
        let a = legacy_piper_voice("en_GB-vctk-medium#12");
        assert_eq!(a, legacy_piper_voice("en_GB-vctk-medium#12"));
        assert!(parse_supertonic(&a).is_ok());
        assert!(legacy_piper_voice("de_DE-thorsten-medium").ends_with("@de"));
    }

    #[test]
    fn empty_text_is_an_error() {
        let t = Tts { model_dir: None, steps: 8 };
        assert!(t.synthesize("   ", "system:en", 1.0).is_err());
    }

    #[test]
    fn missing_model_is_a_clear_error() {
        let t = Tts { model_dir: None, steps: 8 };
        assert!(t.synthesize("hi", "supertonic:F1", 1.0).unwrap_err().contains("fetch-tts"));
    }

    #[test]
    fn trims_padding_but_keeps_a_breath() {
        let mut s = vec![0.0f32; 1000];
        s.extend(vec![0.5f32; 1000]);
        s.extend(vec![0.0f32; 1000]);
        let t = trim_silence(&s, 1000); // 1 kHz: 60 samples before, 150 after
        assert_eq!(t.len(), 60 + 1000 + 150);
    }

    #[test]
    fn wav_header_is_valid() {
        let w = encode_wav(&[0.0, 0.5, -2.0], 44100);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(w.len(), 44 + 6);
        // Peak 2.0 is scaled down to 0.98, not clipped.
        assert_eq!(i16::from_le_bytes([w[48], w[49]]), -32112);
    }

    #[test]
    fn system_voice_makes_a_wav_when_available() {
        let voices = system_voices();
        let Some(v) = voices.iter().find(|v| v.language.starts_with("en")) else {
            eprintln!("no system voices on this machine; skipping");
            return;
        };
        let t = Tts { model_dir: None, steps: 8 };
        let wav = t.synthesize("Hello from Frameling.", &v.id, 1.0).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
    }

    #[test]
    fn supertonic_speaks_when_bundled() {
        let Some(t) = bundled() else {
            eprintln!("Supertonic model not downloaded; skipping");
            return;
        };
        assert_eq!(t.voices().iter().filter(|v| v.engine == "supertonic").count(), 10);
        let a = t.synthesize("Hello from Frameling. This is a test.", "supertonic:F1", 1.0).unwrap();
        assert_eq!(&a[..4], b"RIFF");
        let secs = (a.len() - 44) as f32 / 2.0 / 44100.0;
        assert!((1.0..6.0).contains(&secs), "unexpected length {secs}s");
        // Leading silence is trimmed to a short breath.
        let lead = a[44..].chunks(2).take_while(|b| i16::from_le_bytes([b[0], b[1]]).abs() < 600).count();
        assert!(lead < 44100 / 5, "{lead} samples of leading silence");
        // Repeatable, and faster speech is shorter.
        assert_eq!(a, t.synthesize("Hello from Frameling. This is a test.", "supertonic:F1", 1.0).unwrap());
        let fast = t.synthesize("Hello from Frameling. This is a test.", "supertonic:F1", 1.5).unwrap();
        assert!(fast.len() < a.len());
        let de = t.synthesize("Guten Tag, wie geht es dir?", "supertonic:M2@de", 1.0).unwrap();
        assert!(de.len() > 44 + 44100);
    }
}
