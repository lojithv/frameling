//! Offline text-to-speech.
//!
//! Two engines, both run as separate processes (so their own licences stay
//! separate from Frameling's):
//!
//! * **Piper** (neural, bundled): the `piper` executable plus `.onnx` voice
//!   models shipped inside the app (see `scripts/fetch-tts.mjs`). Users can drop
//!   more Piper voices into the voices folder.
//! * **System voices** (fallback, zero download): `say` on macOS, SAPI on
//!   Windows, eSpeak NG on Linux.
//!
//! Every engine returns a WAV file, which the app imports like any other sound,
//! so lip sync and export work the same for typed, recorded and uploaded lines.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    /// Stable id stored in projects, e.g. `piper:en_US-ljspeech-high` or `piper:en_GB-vctk-medium#12`.
    pub id: String,
    pub name: String,
    /// BCP-47-ish language tag, e.g. `en-US`.
    pub language: String,
    /// "piper" or "system".
    pub engine: String,
    /// Short credit line shown in the UI (licence / dataset).
    pub credit: String,
}

#[derive(Clone, Debug)]
pub struct Tts {
    /// Folder holding the `piper` executable (and its libs / espeak-ng-data).
    pub piper_dir: Option<PathBuf>,
    /// Folders scanned for Piper voices (`*.onnx` + `*.onnx.json`).
    pub voice_dirs: Vec<PathBuf>,
}

/// Friendly names for speakers inside multi-speaker models, from
/// `frameling-voices.json` next to the models:
/// `{ "en_GB-vctk-medium": { "credit": "...", "speakers": { "12": "Ada", ... } } }`
#[derive(Default, Deserialize)]
struct Curated {
    #[serde(default)]
    credit: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    speakers: std::collections::BTreeMap<String, String>,
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

impl Tts {
    /// Look for bundled Piper next to the app (`<base>/tts/piper`, `<base>/tts/voices`).
    pub fn with_bundle(base: &Path) -> Tts {
        let piper_dir = base.join("tts").join("piper");
        Tts {
            piper_dir: piper_dir.join(exe("piper")).is_file().then_some(piper_dir),
            voice_dirs: vec![base.join("tts").join("voices")],
        }
    }

    pub fn piper_exe(&self) -> Option<PathBuf> {
        self.piper_dir.as_ref().map(|d| d.join(exe("piper"))).filter(|p| p.is_file()).or_else(|| which("piper"))
    }

    /// All voices available on this machine. Piper voices come first.
    pub fn voices(&self) -> Vec<Voice> {
        let mut out = Vec::new();
        if self.piper_exe().is_some() {
            out.extend(self.piper_voices());
        }
        out.extend(system_voices());
        out
    }

    fn piper_voices(&self) -> Vec<Voice> {
        let mut out = Vec::new();
        for dir in &self.voice_dirs {
            let curated: std::collections::BTreeMap<String, Curated> = std::fs::read_to_string(dir.join("frameling-voices.json"))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            let mut models: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "onnx"))
                .collect();
            models.sort();
            for model in models {
                let stem = model.file_stem().unwrap().to_string_lossy().to_string();
                let Ok(cfg) = std::fs::read_to_string(model.with_extension("onnx.json")) else { continue };
                let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&cfg) else { continue };
                let lang = cfg["language"]["code"].as_str().unwrap_or("").replace('_', "-");
                let cur = curated.get(&stem);
                let credit = cur.map(|c| c.credit.clone()).unwrap_or_default();
                let base_name = cur.and_then(|c| c.name.clone()).unwrap_or_else(|| pretty_model_name(&stem));
                let speakers = cfg["num_speakers"].as_u64().unwrap_or(1);
                if speakers <= 1 {
                    out.push(Voice { id: format!("piper:{stem}"), name: base_name, language: lang, engine: "piper".into(), credit });
                } else if let Some(c) = cur.filter(|c| !c.speakers.is_empty()) {
                    for (sid, name) in &c.speakers {
                        out.push(Voice { id: format!("piper:{stem}#{sid}"), name: name.clone(), language: lang.clone(), engine: "piper".into(), credit: credit.clone() });
                    }
                } else {
                    for sid in 0..speakers.min(16) {
                        out.push(Voice { id: format!("piper:{stem}#{sid}"), name: format!("{base_name} {}", sid + 1), language: lang.clone(), engine: "piper".into(), credit: credit.clone() });
                    }
                }
            }
        }
        out
    }

    fn find_model(&self, stem: &str) -> Option<PathBuf> {
        if stem.contains(['/', '\\']) || stem.contains("..") {
            return None;
        }
        self.voice_dirs.iter().map(|d| d.join(format!("{stem}.onnx"))).find(|p| p.is_file())
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
            "piper" => {
                let (stem, speaker) = match rest.split_once('#') {
                    Some((s, n)) => (s, Some(n.parse::<u32>().map_err(|_| "bad speaker id")?)),
                    None => (rest, None),
                };
                let model = self.find_model(stem).ok_or_else(|| format!("voice {stem} is not installed"))?;
                let piper = self.piper_exe().ok_or("Piper is not installed")?;
                let out = tmp_path("wav");
                let mut cmd = Command::new(&piper);
                cmd.arg("--model").arg(&model).arg("--output_file").arg(&out).arg("--length_scale").arg(format!("{:.3}", 1.0 / speed));
                if let Some(s) = speaker {
                    cmd.arg("--speaker").arg(s.to_string());
                }
                if let Some(dir) = &self.piper_dir {
                    let data = dir.join("espeak-ng-data");
                    if data.is_dir() {
                        cmd.arg("--espeak_data").arg(data);
                    }
                    cmd.current_dir(dir);
                }
                // One line of input = one utterance; keep the whole line together.
                run(cmd, Some(&format!("{}\n", text.replace(['\r', '\n'], " "))))?;
                read_and_remove(&out)
            }
            "system" => system_say(text, rest, speed),
            _ => Err(format!("unknown voice engine {engine:?}")),
        }
    }
}

fn pretty_model_name(stem: &str) -> String {
    // en_US-ljspeech-high -> "Ljspeech (en_US)"
    let mut parts = stem.split('-');
    let lang = parts.next().unwrap_or("");
    let name = parts.next().unwrap_or(stem);
    let mut c = name.chars();
    let name = match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    };
    format!("{} ({lang})", name.replace('_', " "))
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

    #[test]
    fn rejects_path_tricks_in_voice_ids() {
        let t = Tts { piper_dir: None, voice_dirs: vec![std::env::temp_dir()] };
        assert!(t.find_model("../etc/passwd").is_none());
        assert!(t.synthesize("hi", "piper:../x", 1.0).is_err());
    }

    #[test]
    fn empty_text_is_an_error() {
        let t = Tts { piper_dir: None, voice_dirs: vec![] };
        assert!(t.synthesize("   ", "system:en", 1.0).is_err());
    }

    #[test]
    fn system_voice_makes_a_wav_when_available() {
        let voices = system_voices();
        let Some(v) = voices.iter().find(|v| v.language.starts_with("en")) else {
            eprintln!("no system voices on this machine; skipping");
            return;
        };
        let t = Tts { piper_dir: None, voice_dirs: vec![] };
        let wav = t.synthesize("Hello from Frameling.", &v.id, 1.0).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
    }

    #[test]
    fn piper_voice_folder_is_scanned() {
        let dir = std::env::temp_dir().join(format!("fl-voices-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("en_GB-test-medium.onnx"), b"x").unwrap();
        std::fs::write(dir.join("en_GB-test-medium.onnx.json"), r#"{"language":{"code":"en_GB"},"num_speakers":3}"#).unwrap();
        std::fs::write(dir.join("frameling-voices.json"), r#"{"en_GB-test-medium":{"credit":"CC BY 4.0","speakers":{"0":"Ada","2":"Ben"}}}"#).unwrap();
        let t = Tts { piper_dir: None, voice_dirs: vec![dir.clone()] };
        let v = t.piper_voices();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].id, "piper:en_GB-test-medium#2");
        assert_eq!(v[1].name, "Ben");
        assert_eq!(v[0].language, "en-GB");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
