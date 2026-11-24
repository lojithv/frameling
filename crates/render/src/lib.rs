//! CPU rasteriser for display lists, built on tiny-skia.
//!
//! The same code runs natively (export) and in WebAssembly (editor preview),
//! so what you see while editing is exactly what gets exported.

use base64::Engine as _;
use frameling_core::{Affine, Color, DisplayList, Media, Op, Paint, Seg};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use tiny_skia as sk;

type Images = BTreeMap<String, Arc<Media>>;

/// Decoded pictures, kept across frames: (media id, data length) -> pixels.
fn decoded(m: &Media) -> Option<Arc<sk::Pixmap>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, usize), Option<Arc<sk::Pixmap>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (m.id.clone(), m.data.len());
    if let Some(hit) = cache.lock().ok()?.get(&key) {
        return hit.clone();
    }
    let px = base64::engine::general_purpose::STANDARD
        .decode(m.data.as_bytes())
        .ok()
        .and_then(|b| sk::Pixmap::decode_png(&b).ok())
        .map(Arc::new);
    let mut c = cache.lock().ok()?;
    if c.len() > 64 {
        c.clear();
    }
    c.insert(key, px.clone());
    px
}

fn to_sk_transform(t: &Affine) -> sk::Transform {
    sk::Transform::from_row(t.a, t.b, t.c, t.d, t.e, t.f)
}

fn to_sk_color(c: Color) -> sk::Color {
    sk::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn to_sk_path(p: &frameling_core::Path) -> Option<sk::Path> {
    let mut b = sk::PathBuilder::new();
    let mut open = false;
    for s in &p.0 {
        match *s {
            Seg::M { x, y } => {
                b.move_to(x, y);
                open = true;
            }
            Seg::L { x, y } if open => b.line_to(x, y),
            Seg::Q { x1, y1, x, y } if open => b.quad_to(x1, y1, x, y),
            Seg::C { x1, y1, x2, y2, x, y } if open => b.cubic_to(x1, y1, x2, y2, x, y),
            Seg::Z if open => b.close(),
            _ => {}
        }
    }
    b.finish()
}

fn make_paint(p: &Paint) -> Option<sk::Paint<'static>> {
    let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
    match p {
        Paint::Solid { color } => {
            if color.a == 0 {
                return None;
            }
            paint.set_color(to_sk_color(*color));
        }
        Paint::Linear { x0, y0, x1, y1, stops } => {
            let stops: Vec<sk::GradientStop> = stops.iter().map(|(o, c)| sk::GradientStop::new(*o, to_sk_color(*c))).collect();
            let shader = sk::LinearGradient::new(
                sk::Point::from_xy(*x0, *y0),
                sk::Point::from_xy(*x1, *y1),
                stops,
                sk::SpreadMode::Pad,
                sk::Transform::identity(),
            )?;
            paint.shader = shader;
        }
    }
    Some(paint)
}

fn draw_ops(pm: &mut sk::Pixmap, ops: &[Op], t: Affine, images: &Images) {
    for op in ops {
        match op {
            Op::Fill { path, paint } => {
                let (Some(path), Some(paint)) = (to_sk_path(path), make_paint(paint)) else { continue };
                pm.fill_path(&path, &paint, sk::FillRule::Winding, to_sk_transform(&t), None);
            }
            Op::Stroke { path, color, width } => {
                if color.a == 0 || *width <= 0.0 {
                    continue;
                }
                let Some(path) = to_sk_path(path) else { continue };
                let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
                paint.set_color(to_sk_color(*color));
                let stroke = sk::Stroke { width: *width, line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() };
                pm.stroke_path(&path, &paint, &stroke, to_sk_transform(&t), None);
            }
            Op::Image { media, x, y, w, h, r } => {
                let Some(img) = images.get(media).and_then(|m| decoded(m)) else { continue };
                let Some(shape) = to_sk_path(&frameling_core::Path::round_rect(*x, *y, w.max(0.01), h.max(0.01), *r)) else { continue };
                let (iw, ih) = (img.width() as f32, img.height() as f32);
                let pattern_t = sk::Transform::from_row(w / iw, 0.0, 0.0, h / ih, *x, *y);
                let paint = sk::Paint {
                    anti_alias: true,
                    shader: sk::Pattern::new(sk::Pixmap::as_ref(&img), sk::SpreadMode::Pad, sk::FilterQuality::Bilinear, 1.0, pattern_t),
                    ..Default::default()
                };
                pm.fill_path(&shape, &paint, sk::FillRule::Winding, to_sk_transform(&t), None);
            }
            Op::Group { transform, opacity, children } => {
                if *opacity <= 0.0 || children.is_empty() {
                    continue;
                }
                let inner = t.then(transform);
                if *opacity >= 0.999 {
                    draw_ops(pm, children, inner, images);
                } else {
                    // Group opacity: draw into a layer, then composite once.
                    let Some(mut layer) = sk::Pixmap::new(pm.width(), pm.height()) else { continue };
                    draw_ops(&mut layer, children, inner, images);
                    let paint = sk::PixmapPaint { opacity: *opacity, ..Default::default() };
                    pm.draw_pixmap(0, 0, layer.as_ref(), &paint, sk::Transform::identity(), None);
                }
            }
        }
    }
}

/// Rasterise a display list at `width` x `height` pixels.
/// Returns premultiplied RGBA bytes (fully opaque when the list has a background).
pub fn render(dl: &DisplayList, width: u32, height: u32) -> Vec<u8> {
    let mut pm = sk::Pixmap::new(width.max(1), height.max(1)).expect("valid size");
    if let Some(bg) = dl.background {
        pm.fill(to_sk_color(bg));
    }
    let root = Affine::scale(width as f32 / dl.width.max(1) as f32, height as f32 / dl.height.max(1) as f32);
    draw_ops(&mut pm, &dl.ops, root, &dl.images);
    pm.take()
}

/// Render and encode as PNG (used by the CLI for stills and thumbnails).
pub fn render_png(dl: &DisplayList, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let data = render(dl, width, height);
    let pm = sk::Pixmap::from_vec(data, sk::IntSize::from_wh(width.max(1), height.max(1)).ok_or("bad size")?).ok_or("bad pixmap")?;
    pm.encode_png().map_err(|e| e.to_string())
}
