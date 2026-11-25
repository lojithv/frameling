//! Text layout. Glyphs are turned into vector paths with embedded fonts, so
//! text goes through exactly the same renderer as everything else and looks
//! identical in preview and export on every platform.
//!
//! [`layout_glyphs`] keeps every glyph separate (with its letter and word
//! index), which is what letter-by-letter animation and word-by-word caption
//! highlighting need.

use crate::display::Path;
use crate::model::Font;
use std::sync::OnceLock;

/// Inter SemiBold, Latin subset. Licences: see assets/fonts/README.md.
static SANS: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold-subset.otf");
static HEAVY: &[u8] = include_bytes!("../../../assets/fonts/Figtree-Black.ttf");
static ROUNDED: &[u8] = include_bytes!("../../../assets/fonts/Poppins-Bold.ttf");
static MONO: &[u8] = include_bytes!("../../../assets/fonts/RobotoMono-Bold.ttf");
static SERIF: &[u8] = include_bytes!("../../../assets/fonts/KaTeX_Main-Bold.ttf");

fn face(font: Font) -> &'static ttf_parser::Face<'static> {
    static FACES: [OnceLock<ttf_parser::Face<'static>>; 5] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let (i, data) = match font {
        Font::Sans => (0, SANS),
        Font::Heavy => (1, HEAVY),
        Font::Rounded => (2, ROUNDED),
        Font::Mono => (3, MONO),
        Font::Serif => (4, SERIF),
    };
    FACES[i].get_or_init(|| ttf_parser::Face::parse(data, 0).expect("embedded font parses"))
}

struct Builder<'a> {
    p: &'a mut Path,
    s: f32,
    x: f32,
    y: f32,
}

impl ttf_parser::OutlineBuilder for Builder<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.p.move_to(self.x + x * self.s, self.y - y * self.s);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.p.line_to(self.x + x * self.s, self.y - y * self.s);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.p.quad_to(self.x + x1 * self.s, self.y - y1 * self.s, self.x + x * self.s, self.y - y * self.s);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.p.cubic_to(
            self.x + x1 * self.s,
            self.y - y1 * self.s,
            self.x + x2 * self.s,
            self.y - y2 * self.s,
            self.x + x * self.s,
            self.y - y * self.s,
        );
    }
    fn close(&mut self) {
        self.p.close();
    }
}

/// Laid-out text block.
pub struct TextBlock {
    /// Glyph outlines, positioned so the block's centre is at (0, 0).
    pub path: Path,
    pub width: f32,
    pub height: f32,
}

fn glyph(f: &ttf_parser::Face<'_>, ch: char) -> Option<ttf_parser::GlyphId> {
    f.glyph_index(ch).or_else(|| f.glyph_index('?'))
}

fn advance(font: Font, ch: char, scale: f32) -> f32 {
    let f = face(font);
    glyph(f, ch).and_then(|g| f.glyph_hor_advance(g)).map_or(0.0, |a| a as f32 * scale)
}

fn units(font: Font, size: f32) -> f32 {
    size / face(font).units_per_em() as f32
}

/// Width of a single line of text.
pub fn measure_in(font: Font, s: &str, size: f32) -> f32 {
    measure_spaced(font, s, size, 0.0)
}

/// Width of a line with `extra` px added to every space.
fn measure_spaced(font: Font, s: &str, size: f32, extra: f32) -> f32 {
    let scale = units(font, size);
    s.chars().map(|c| advance(font, c, scale) + if c == ' ' { extra } else { 0.0 }).sum()
}

/// Width of a single line of text in the default typeface.
pub fn measure(s: &str, size: f32) -> f32 {
    measure_in(Font::Sans, s, size)
}

/// Greedy word wrap to `max_width` (explicit newlines are kept).
pub fn wrap_in(font: Font, text: &str, size: f32, max_width: f32) -> Vec<String> {
    wrap_spaced(font, text, size, max_width, 0.0)
}

fn wrap_spaced(font: Font, text: &str, size: f32, max_width: f32, extra: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && measure_spaced(font, &candidate, size, extra) > max_width {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Greedy word wrap in the default typeface.
pub fn wrap(text: &str, size: f32, max_width: f32) -> Vec<String> {
    wrap_in(Font::Sans, text, size, max_width)
}

/// One positioned glyph.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub ch: char,
    /// Reading order of the letter, counting spaces and line breaks (the
    /// typewriter counts the same way).
    pub index: usize,
    /// Which word it belongs to (0-based).
    pub word: usize,
    pub line: usize,
    /// Left edge and baseline, block-centred coordinates.
    pub x: f32,
    pub baseline: f32,
    pub advance: f32,
    /// Outline in block-centred coordinates (empty for spaces).
    pub path: Path,
}

impl Glyph {
    /// A point in the middle of the letter, to scale or spin it around.
    pub fn center(&self, size: f32) -> (f32, f32) {
        (self.x + self.advance / 2.0, self.baseline - size * 0.36)
    }
}

/// Text laid out glyph by glyph, centred on (0, 0).
pub struct Layout {
    pub glyphs: Vec<Glyph>,
    pub width: f32,
    pub height: f32,
    pub size: f32,
    pub ascent: f32,
    pub descent: f32,
    /// Letter units (see [`Glyph::index`]) and words in the text.
    pub letters: usize,
    pub words: usize,
}

impl Layout {
    /// All glyph outlines as one path.
    pub fn path(&self) -> Path {
        let mut p = Path::new();
        for g in &self.glyphs {
            p.extend(&g.path);
        }
        p
    }
}

/// Lay out centred, wrapped text, keeping each glyph separate.
pub fn layout_glyphs(text: &str, font: Font, size: f32, max_width: f32) -> Layout {
    layout_spaced(text, font, size, max_width, 0.0)
}

/// Like [`layout_glyphs`], with `extra` px between words (captions, where
/// the spoken word grows).
pub fn layout_spaced(text: &str, font: Font, size: f32, max_width: f32, extra: f32) -> Layout {
    let f = face(font);
    let scale = units(font, size);
    let line_h = size * 1.28;
    let ascent = f.ascender() as f32 * scale;
    let descent = -(f.descender() as f32) * scale;
    let lines = wrap_spaced(font, text, size, max_width, extra);
    let widths: Vec<f32> = lines.iter().map(|l| measure_spaced(font, l, size, extra)).collect();
    let width = widths.iter().cloned().fold(0.0, f32::max);
    let height = line_h * (lines.len().max(1) as f32 - 1.0) + ascent + descent;
    let top = -height / 2.0;
    let mut glyphs = Vec::new();
    let mut index = 0;
    let mut word = 0usize;
    let mut words = 0usize;
    for (li, line) in lines.iter().enumerate() {
        let mut x = -widths[li] / 2.0;
        let baseline = top + ascent + li as f32 * line_h;
        let mut in_word = false;
        for ch in line.chars() {
            let adv = advance(font, ch, scale) + if ch == ' ' { extra } else { 0.0 };
            let mut path = Path::new();
            if ch.is_whitespace() {
                in_word = false;
            } else {
                if !in_word {
                    word = words;
                    words += 1;
                    in_word = true;
                }
                if let Some(g) = glyph(f, ch) {
                    let mut b = Builder { p: &mut path, s: scale, x, y: baseline };
                    f.outline_glyph(g, &mut b);
                }
            }
            glyphs.push(Glyph { ch, index, word, line: li, x, baseline, advance: adv, path });
            index += 1;
            x += adv;
        }
        index += 1; // the line break / space
    }
    Layout { glyphs, width, height, size, ascent, descent, letters: index.saturating_sub(1), words }
}

/// Lay out centred, wrapped text. `visible` limits how many characters are
/// drawn (typewriter effect) without changing the layout.
pub fn layout(text: &str, size: f32, max_width: f32, visible: Option<usize>) -> TextBlock {
    let l = layout_glyphs(text, Font::Sans, size, max_width);
    let mut path = Path::new();
    let limit = visible.unwrap_or(usize::MAX);
    for g in l.glyphs.iter().filter(|g| g.index < limit) {
        path.extend(&g.path);
    }
    TextBlock { path, width: l.width, height: l.height }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_font_lays_out() {
        for font in [Font::Sans, Font::Heavy, Font::Rounded, Font::Mono, Font::Serif] {
            let l = layout_glyphs("Hello big world", font, 40.0, 2000.0);
            assert_eq!(l.words, 3, "{font:?}");
            assert!(l.width > 100.0, "{font:?}");
            assert!(l.glyphs.iter().filter(|g| !g.path.is_empty()).count() == 13, "{font:?}");
        }
    }

    #[test]
    fn words_and_letters_are_counted_across_lines() {
        let l = layout_glyphs("one two three four", Font::Sans, 40.0, 120.0);
        assert!(l.glyphs.iter().map(|g| g.line).max().unwrap() >= 1, "wraps");
        assert_eq!(l.words, 4);
        let last = l.glyphs.iter().filter(|g| g.word == 3).count();
        assert_eq!(last, 4);
    }
}
