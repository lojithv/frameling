//! CPU rasteriser for display lists, built on tiny-skia.
//!
//! The same code runs natively (export) and in WebAssembly (editor preview),
//! so what you see while editing is exactly what gets exported.

use frameling_core::{Affine, Color, DisplayList, Op, Paint, Seg};
use tiny_skia as sk;

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

fn draw_ops(pm: &mut sk::Pixmap, ops: &[Op], t: Affine) {
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
            Op::Group { transform, opacity, children } => {
                if *opacity <= 0.0 || children.is_empty() {
                    continue;
                }
                let inner = t.then(transform);
                if *opacity >= 0.999 {
                    draw_ops(pm, children, inner);
                } else {
                    // Group opacity: draw into a layer, then composite once.
                    let Some(mut layer) = sk::Pixmap::new(pm.width(), pm.height()) else { continue };
                    draw_ops(&mut layer, children, inner);
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
    draw_ops(&mut pm, &dl.ops, root);
    pm.take()
}

/// Render and encode as PNG (used by the CLI for stills and thumbnails).
pub fn render_png(dl: &DisplayList, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let data = render(dl, width, height);
    let pm = sk::Pixmap::from_vec(data, sk::IntSize::from_wh(width.max(1), height.max(1)).ok_or("bad size")?).ok_or("bad pixmap")?;
    pm.encode_png().map_err(|e| e.to_string())
}
