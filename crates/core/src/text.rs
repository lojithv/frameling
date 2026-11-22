//! Text layout. Glyphs are turned into vector paths with an embedded font, so
//! text goes through exactly the same renderer as everything else and looks
//! identical in preview and export on every platform.

use crate::display::Path;
use std::sync::OnceLock;

/// Inter SemiBold, Latin subset (SIL Open Font License 1.1, see assets/fonts/OFL.txt).
static FONT_DATA: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold-subset.otf");

fn face() -> &'static ttf_parser::Face<'static> {
    static FACE: OnceLock<ttf_parser::Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| ttf_parser::Face::parse(FONT_DATA, 0).expect("embedded font parses"))
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

fn advance(ch: char, scale: f32) -> f32 {
    let f = face();
    f.glyph_index(ch)
        .or_else(|| f.glyph_index('?'))
        .and_then(|g| f.glyph_hor_advance(g))
        .map_or(0.0, |a| a as f32 * scale)
}

/// Width of a single line of text.
pub fn measure(s: &str, size: f32) -> f32 {
    let scale = size / face().units_per_em() as f32;
    s.chars().map(|c| advance(c, scale)).sum()
}

/// Greedy word wrap to `max_width` (explicit newlines are kept).
pub fn wrap(text: &str, size: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && measure(&candidate, size) > max_width {
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

/// Lay out centred, wrapped text. `visible` limits how many characters are
/// drawn (typewriter effect) without changing the layout.
pub fn layout(text: &str, size: f32, max_width: f32, visible: Option<usize>) -> TextBlock {
    let f = face();
    let scale = size / f.units_per_em() as f32;
    let line_h = size * 1.28;
    let ascent = f.ascender() as f32 * scale;
    let descent = -(f.descender() as f32) * scale;
    let lines = wrap(text, size, max_width);
    let widths: Vec<f32> = lines.iter().map(|l| measure(l, size)).collect();
    let width = widths.iter().cloned().fold(0.0, f32::max);
    let height = line_h * (lines.len().max(1) as f32 - 1.0) + ascent + descent;
    let mut path = Path::new();
    let mut remaining = visible.unwrap_or(usize::MAX);
    let top = -height / 2.0;
    for (i, line) in lines.iter().enumerate() {
        let mut x = -widths[i] / 2.0;
        let baseline = top + ascent + i as f32 * line_h;
        for ch in line.chars() {
            if remaining == 0 {
                break;
            }
            remaining -= 1;
            if let Some(g) = f.glyph_index(ch).or_else(|| f.glyph_index('?')) {
                let mut b = Builder { p: &mut path, s: scale, x, y: baseline };
                f.outline_glyph(g, &mut b);
            }
            x += advance(ch, scale);
        }
        remaining = remaining.saturating_sub(1); // the line break / space
    }
    TextBlock { path, width, height }
}
