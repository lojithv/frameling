//! Supertonic 3 (Supertone Inc.) running in-process on ONNX Runtime.
//!
//! The pipeline follows Supertone's MIT-licensed sample code: text → unicode
//! ids → duration predictor → text encoder → flow-matching vector estimator
//! (a few denoising steps) → vocoder → 44.1 kHz mono audio.
//!
//! The model weights are NOT part of this crate. They are under the
//! BigScience OpenRAIL-M licence and are downloaded by `npm run fetch-tts`
//! into `<bundle>/tts/supertonic` (see docs/VOICES.md).

use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use unicode_normalization::UnicodeNormalization;

/// The ten voice styles Supertone ships. Names are Frameling's own.
pub const VOICES: &[(&str, &str)] = &[
    ("F1", "Hazel"),
    ("F2", "Juniper"),
    ("F3", "Marigold"),
    ("F4", "Clover"),
    ("F5", "Saffron"),
    ("M1", "Rowan"),
    ("M2", "Bramble"),
    ("M3", "Cedar"),
    ("M4", "Linden"),
    ("M5", "Thistle"),
];

/// Languages the model speaks (ISO 639-1).
pub const LANGS: &[&str] = &[
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi", "hr", "hu", "id", "it", "lt", "lv", "nl", "pl",
    "pt", "ro", "ru", "sk", "sl", "sv", "tr", "uk", "vi",
];

/// Denoising steps. 8 is Supertone's default; 10 is a touch cleaner and still
/// several times faster than real time on a laptop CPU.
pub const DEFAULT_STEPS: usize = 10;
/// Supertonic's natural pace is `speed = 1.05`; Frameling's 1.0 maps to that.
const BASE_SPEED: f32 = 1.05;
/// Gap between sentences that were synthesised separately.
const CHUNK_GAP_S: f32 = 0.25;

pub struct Model {
    sample_rate: u32,
    base_chunk_size: usize,
    chunk_compress: usize,
    latent_dim: usize,
    indexer: Vec<i64>,
    dp: Session,
    text_enc: Session,
    vector_est: Session,
    vocoder: Session,
    styles: HashMap<String, Style>,
    dir: PathBuf,
}

#[derive(Clone)]
struct Style {
    ttl: (Vec<i64>, Vec<f32>),
    dp: (Vec<i64>, Vec<f32>),
}

/// True when `dir` holds a complete model (graphs, config and at least one voice).
pub fn is_model_dir(dir: &Path) -> bool {
    ["duration_predictor.onnx", "text_encoder.onnx", "vector_estimator.onnx", "vocoder.onnx", "tts.json", "unicode_indexer.json"]
        .iter()
        .all(|f| dir.join("onnx").join(f).is_file())
}

/// Voice styles present in the model folder, in the order of [`VOICES`].
pub fn installed_voices(dir: &Path) -> Vec<(&'static str, &'static str)> {
    VOICES.iter().copied().filter(|(id, _)| dir.join("voice_styles").join(format!("{id}.json")).is_file()).collect()
}

fn err<E: std::fmt::Display>(what: &str) -> impl Fn(E) -> String + '_ {
    move |e| format!("{what}: {e}")
}

/// Loading the four graphs takes a second or two and ~1 GB of RAM, so each
/// model folder is loaded once per process and shared.
pub fn shared(dir: &Path) -> Result<Arc<Mutex<Model>>, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<Model>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut map = cache.lock().map_err(|_| "speech engine lock poisoned")?;
    if let Some(m) = map.get(dir) {
        return Ok(m.clone());
    }
    let m = Arc::new(Mutex::new(Model::load(dir)?));
    map.insert(dir.to_path_buf(), m.clone());
    Ok(m)
}

fn flatten(v: &Value, out: &mut Vec<f32>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| flatten(x, out)),
        Value::Number(n) => out.push(n.as_f64().unwrap_or(0.0) as f32),
        _ => {}
    }
}

fn tensor_json(v: &Value, what: &str) -> Result<(Vec<i64>, Vec<f32>), String> {
    let dims: Vec<i64> = v["dims"].as_array().ok_or(format!("voice style: {what}.dims missing"))?.iter().map(|d| d.as_i64().unwrap_or(0)).collect();
    let mut data = Vec::new();
    flatten(&v["data"], &mut data);
    let n: i64 = dims.iter().product();
    if dims.len() < 3 || n as usize != data.len() {
        return Err(format!("voice style: {what} has {} values for shape {dims:?}", data.len()));
    }
    Ok((dims, data))
}

fn read_json(p: &Path) -> Result<Value, String> {
    let s = std::fs::read_to_string(p).map_err(err(&format!("could not read {}", p.display())))?;
    serde_json::from_str(&s).map_err(err(&format!("bad JSON in {}", p.display())))
}

impl Model {
    pub fn load(dir: &Path) -> Result<Model, String> {
        let onnx = dir.join("onnx");
        let cfg = read_json(&onnx.join("tts.json"))?;
        let get = |path: &[&str]| -> Result<usize, String> {
            let mut v = &cfg;
            for k in path {
                v = &v[*k];
            }
            v.as_u64().map(|x| x as usize).ok_or(format!("tts.json: missing {}", path.join(".")))
        };
        let sample_rate = get(&["ae", "sample_rate"])? as u32;
        let base_chunk_size = get(&["ae", "base_chunk_size"])?;
        let chunk_compress = get(&["ttl", "chunk_compress_factor"])?;
        let latent_dim = get(&["ttl", "latent_dim"])?;

        // unicode_indexer.json maps code point -> token id (-1 = unknown). It is a
        // dense array in the published model; accept an object form too.
        let idx = read_json(&onnx.join("unicode_indexer.json"))?;
        let indexer: Vec<i64> = match &idx {
            Value::Array(a) => a.iter().map(|v| v.as_i64().unwrap_or(-1)).collect(),
            Value::Object(o) => {
                let max = o.keys().filter_map(|k| k.parse::<usize>().ok()).max().unwrap_or(0);
                let mut v = vec![-1; max + 1];
                for (k, x) in o {
                    if let Ok(i) = k.parse::<usize>() {
                        v[i] = x.as_i64().unwrap_or(-1);
                    }
                }
                v
            }
            _ => return Err("unicode_indexer.json has an unexpected shape".into()),
        };

        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        let open = |name: &str| -> Result<Session, String> {
            Session::builder()
                .and_then(|b| b.with_optimization_level(GraphOptimizationLevel::Level3))
                .and_then(|b| b.with_intra_threads(threads))
                .and_then(|b| b.commit_from_file(onnx.join(name)))
                .map_err(err(&format!("could not load {name}")))
        };
        Ok(Model {
            sample_rate,
            base_chunk_size,
            chunk_compress,
            latent_dim,
            indexer,
            dp: open("duration_predictor.onnx")?,
            text_enc: open("text_encoder.onnx")?,
            vector_est: open("vector_estimator.onnx")?,
            vocoder: open("vocoder.onnx")?,
            styles: HashMap::new(),
            dir: dir.to_path_buf(),
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn style(&mut self, voice: &str) -> Result<Style, String> {
        if !VOICES.iter().any(|(id, _)| *id == voice) {
            return Err(format!("unknown Supertonic voice {voice:?}"));
        }
        if let Some(s) = self.styles.get(voice) {
            return Ok(s.clone());
        }
        let j = read_json(&self.dir.join("voice_styles").join(format!("{voice}.json")))?;
        let s = Style { ttl: tensor_json(&j["style_ttl"], "style_ttl")?, dp: tensor_json(&j["style_dp"], "style_dp")? };
        self.styles.insert(voice.to_string(), s.clone());
        Ok(s)
    }

    /// Speak `text` in `lang` with voice style `voice` (`F1`…`M5`). `speed` is
    /// Frameling's (1.0 = normal). Returns mono samples in -1..1.
    pub fn synthesize(&mut self, text: &str, lang: &str, voice: &str, speed: f32, steps: usize) -> Result<Vec<f32>, String> {
        if !LANGS.contains(&lang) {
            return Err(format!("Supertonic does not speak {lang:?}; it supports {}", LANGS.join(", ")));
        }
        let style = self.style(voice)?;
        let max_len = if lang == "ko" || lang == "ja" { 120 } else { 300 };
        let chunks = chunk_text(text, max_len);
        if chunks.is_empty() {
            return Err("nothing to say".into());
        }
        // Same text + voice + speed gives the same audio, so re-voicing is repeatable.
        let mut rng = Rng::new(hash(&[text, lang, voice, &format!("{speed:.3}")]));
        let gap = vec![0.0f32; (CHUNK_GAP_S * self.sample_rate as f32) as usize];
        let mut out = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            if i > 0 {
                out.extend_from_slice(&gap);
            }
            let pre = preprocess(c, lang);
            out.extend(self.infer(&pre, &style, speed * BASE_SPEED, steps.max(1), &mut rng)?);
        }
        Ok(out)
    }

    fn infer(&mut self, text: &str, style: &Style, speed: f32, steps: usize, rng: &mut Rng) -> Result<Vec<f32>, String> {
        let ids: Vec<i64> = text.chars().map(|c| self.indexer.get(c as usize).copied().unwrap_or(-1)).collect();
        let n = ids.len();
        let ids_t = || Tensor::from_array((vec![1usize, n], ids.clone())).map_err(err("text ids"));
        let mask_t = || Tensor::from_array((vec![1usize, 1, n], vec![1.0f32; n])).map_err(err("text mask"));
        let st = |t: &(Vec<i64>, Vec<f32>)| Tensor::from_array((t.0.iter().map(|&d| d as usize).collect::<Vec<_>>(), t.1.clone())).map_err(err("voice style"));

        // 1. How long will it take to say?
        let dp_out = self
            .dp
            .run(ort::inputs! { "text_ids" => ids_t()?, "style_dp" => st(&style.dp)?, "text_mask" => mask_t()? })
            .map_err(err("duration predictor"))?;
        let (_, dur) = dp_out["duration"].try_extract_tensor::<f32>().map_err(err("duration"))?;
        let duration = (dur[0] / speed).max(0.05);
        drop(dp_out);

        // 2. Encode the text in the voice's style.
        let te_out = self
            .text_enc
            .run(ort::inputs! { "text_ids" => ids_t()?, "style_ttl" => st(&style.ttl)?, "text_mask" => mask_t()? })
            .map_err(err("text encoder"))?;
        let (emb_shape, emb) = te_out["text_emb"].try_extract_tensor::<f32>().map_err(err("text embedding"))?;
        let emb_shape: Vec<usize> = emb_shape.iter().map(|&d| d as usize).collect();
        let emb = emb.to_vec();
        drop(te_out);

        // 3. Start from noise in latent space and denoise it towards speech.
        let wav_len = (duration * self.sample_rate as f32) as usize;
        let chunk = self.base_chunk_size * self.chunk_compress;
        let latent_len = wav_len.div_ceil(chunk).max(1);
        let dim = self.latent_dim * self.chunk_compress;
        let mut xt: Vec<f32> = (0..dim * latent_len).map(|_| rng.normal()).collect();
        for step in 0..steps {
            let out = self
                .vector_est
                .run(ort::inputs! {
                    "noisy_latent" => Tensor::from_array((vec![1usize, dim, latent_len], xt.clone())).map_err(err("latent"))?,
                    "text_emb" => Tensor::from_array((emb_shape.clone(), emb.clone())).map_err(err("text embedding"))?,
                    "style_ttl" => st(&style.ttl)?,
                    "latent_mask" => Tensor::from_array((vec![1usize, 1, latent_len], vec![1.0f32; latent_len])).map_err(err("latent mask"))?,
                    "text_mask" => mask_t()?,
                    "current_step" => Tensor::from_array((vec![1usize], vec![step as f32])).map_err(err("step"))?,
                    "total_step" => Tensor::from_array((vec![1usize], vec![steps as f32])).map_err(err("steps"))?,
                })
                .map_err(err("vector estimator"))?;
            let (_, d) = out["denoised_latent"].try_extract_tensor::<f32>().map_err(err("denoised latent"))?;
            let n = xt.len();
            xt.copy_from_slice(&d[..n]);
        }

        // 4. Latent -> waveform, trimmed to the predicted length.
        let voc = self
            .vocoder
            .run(ort::inputs! { "latent" => Tensor::from_array((vec![1usize, dim, latent_len], xt)).map_err(err("latent"))? })
            .map_err(err("vocoder"))?;
        let (_, wav) = voc["wav_tts"].try_extract_tensor::<f32>().map_err(err("audio"))?;
        Ok(wav[..wav_len.min(wav.len())].to_vec())
    }
}

// ---------------------------------------------------------------- text

/// Same clean-up Supertone's reference code does, then wrap in language tags.
pub fn preprocess(text: &str, lang: &str) -> String {
    let mut t: String = text.nfkd().filter(|c| !is_emoji(*c) && !"♥☆♡©\\".contains(*c)).collect();
    for (a, b) in [
        ("–", "-"), ("‑", "-"), ("—", "-"), ("_", " "), ("\u{201C}", "\""), ("\u{201D}", "\""), ("\u{2018}", "'"), ("\u{2019}", "'"),
        ("´", "'"), ("`", "'"), ("[", " "), ("]", " "), ("|", " "), ("/", " "), ("#", " "), ("→", " "), ("←", " "),
        ("@", " at "), ("e.g.,", "for example, "), ("i.e.,", "that is, "),
    ] {
        t = t.replace(a, b);
    }
    for (a, b) in [(" ,", ","), (" .", "."), (" !", "!"), (" ?", "?"), (" ;", ";"), (" :", ":"), (" '", "'"), ("\"\"", "\""), ("''", "'")] {
        t = t.replace(a, b);
    }
    let mut t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if !t.ends_with(['.', '!', '?', ';', ':', ',', '\'', '"', ')', ']', '}', '…', '。', '」', '』', '】', '〉', '》', '›', '»', '>']) {
        t.push('.');
    }
    format!("<{lang}>{t}</{lang}>")
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x1F1E6..=0x1F1FF)
}

const ABBREVIATIONS: &[&str] = &["Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "Sr.", "Jr.", "Ph.D.", "etc.", "e.g.", "i.e.", "vs.", "Inc.", "Ltd.", "Co.", "Corp.", "St.", "Ave.", "Blvd."];

/// Split into sentences (keeping "Dr. Smith" together).
fn sentences(p: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = p.chars().peekable();
    while let Some(c) = chars.next() {
        cur.push(c);
        let end = matches!(c, '.' | '!' | '?' | '。' | '！' | '？');
        if end && chars.peek().is_none_or(|n| n.is_whitespace()) {
            let last_word = cur.split_whitespace().last().unwrap_or("");
            let initial = last_word.len() == 2 && last_word.starts_with(|ch: char| ch.is_ascii_uppercase());
            if c != '.' || !(ABBREVIATIONS.contains(&last_word) || initial) {
                out.push(cur.trim().to_string());
                cur.clear();
            }
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Paragraphs → sentences → chunks of at most `max_len` characters.
pub fn chunk_text(text: &str, max_len: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let push_long = |s: &str, chunks: &mut Vec<String>| {
        let mut cur = String::new();
        for w in s.split_whitespace() {
            if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > max_len {
                chunks.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(w);
        }
        if !cur.is_empty() {
            chunks.push(cur);
        }
    };
    let text = text.replace("\r\n", "\n");
    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        let mut cur = String::new();
        for s in sentences(&para.replace('\n', " ")) {
            if cur.is_empty() {
                cur = s;
            } else if cur.chars().count() + 1 + s.chars().count() <= max_len {
                cur.push(' ');
                cur.push_str(&s);
            } else {
                push_long(&std::mem::take(&mut cur), &mut chunks);
                cur = s;
            }
        }
        if !cur.is_empty() {
            push_long(&cur, &mut chunks);
        }
    }
    chunks
}

// ---------------------------------------------------------------- noise

fn hash(parts: &[&str]) -> u64 {
    // FNV-1a: stable across builds and platforms (std's hasher is not).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain([0xff]) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

struct Rng(u64, Option<f32>);
impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed | 1, None)
    }
    fn uniform(&mut self) -> f64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64) / (1u64 << 53) as f64
    }
    /// Standard normal (Box-Muller).
    fn normal(&mut self) -> f32 {
        if let Some(z) = self.1.take() {
            return z;
        }
        let u1 = self.uniform().max(1e-7);
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let a = 2.0 * std::f64::consts::PI * u2;
        self.1 = Some((r * a.sin()) as f32);
        (r * a.cos()) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_wraps_and_punctuates() {
        assert_eq!(preprocess("Hello  “world” — hi", "en"), "<en>Hello \"world\" - hi.</en>");
        assert_eq!(preprocess("Done!", "fr"), "<fr>Done!</fr>");
    }

    #[test]
    fn chunks_keep_titles_together() {
        let c = chunk_text("Dr. Smith is here. He said hi! Did you hear?", 25);
        assert_eq!(c, vec!["Dr. Smith is here.", "He said hi! Did you hear?"]);
    }

    #[test]
    fn long_sentences_split_on_words() {
        let s = "word ".repeat(100);
        assert!(chunk_text(&s, 50).iter().all(|c| c.len() <= 50));
    }

    #[test]
    fn noise_is_deterministic() {
        let a: Vec<f32> = { let mut r = Rng::new(hash(&["hi"])); (0..4).map(|_| r.normal()).collect() };
        let b: Vec<f32> = { let mut r = Rng::new(hash(&["hi"])); (0..4).map(|_| r.normal()).collect() };
        assert_eq!(a, b);
    }
}
