use std::path::Path;

use eframe::egui::{self, Color32, TextureId};

pub fn write_cpu_png(ctx: &egui::Context, path: &Path) -> Result<(), String> {
    let ppp = ctx.pixels_per_point();
    let screen = ctx.input(|i| i.screen_rect);
    let w = (screen.width() * ppp).round().max(1.0) as u32;
    let h = (screen.height() * ppp).round().max(1.0) as u32;
    let bg = ctx.style().visuals.window_fill;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let bg_px = bg.to_array();
    for chunk in pixels.as_chunks_mut::<4>().0 {
        *chunk = bg_px;
    }
    let shapes = ctx.graphics(|g| {
        let mut clone = g.clone();
        clone.drain(&[], &egui::ahash::AHashMap::default())
    });
    let font_image = ctx.fonts(|f| f.image().clone());
    let clipped = ctx.tessellate(shapes, ppp);
    for clipped_prim in clipped {
        paint_primitive(&mut pixels, w, h, ppp, &clipped_prim, &font_image);
    }
    image::save_buffer(path, &pixels, w, h, image::ColorType::Rgba8).map_err(|e| e.to_string())
}

fn paint_primitive(
    pixels: &mut [u8],
    w: u32,
    h: u32,
    ppp: f32,
    clip: &egui::ClippedPrimitive,
    font_image: &egui::FontImage,
) {
    let clip_px = rect_to_px(clip.clip_rect, ppp);
    match &clip.primitive {
        egui::epaint::Primitive::Mesh(mesh) => {
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                return;
            }
            let font_atlas = mesh.texture_id == TextureId::default()
                || matches!(mesh.texture_id, TextureId::Managed(0));
            for chunk in mesh.indices.as_chunks::<3>().0 {
                let v0 = &mesh.vertices[chunk[0] as usize];
                let v1 = &mesh.vertices[chunk[1] as usize];
                let v2 = &mesh.vertices[chunk[2] as usize];
                rasterize_tri(
                    pixels,
                    w,
                    h,
                    ppp,
                    clip_px,
                    v0,
                    v1,
                    v2,
                    font_atlas.then_some(font_image),
                );
            }
        }
        egui::epaint::Primitive::Callback(_) => {}
    }
}

#[derive(Clone, Copy)]
struct Px {
    x: f32,
    y: f32,
}

#[allow(clippy::too_many_arguments)]
fn rasterize_tri(
    pixels: &mut [u8],
    w: u32,
    h: u32,
    ppp: f32,
    clip: egui::Rect,
    v0: &egui::epaint::Vertex,
    v1: &egui::epaint::Vertex,
    v2: &egui::epaint::Vertex,
    font_image: Option<&egui::FontImage>,
) {
    let p0 = Px {
        x: v0.pos.x * ppp,
        y: v0.pos.y * ppp,
    };
    let p1 = Px {
        x: v1.pos.x * ppp,
        y: v1.pos.y * ppp,
    };
    let p2 = Px {
        x: v2.pos.x * ppp,
        y: v2.pos.y * ppp,
    };
    let min_x = p0.x.min(p1.x).min(p2.x).max(clip.min.x).floor() as i32;
    let max_x = p0.x.max(p1.x).max(p2.x).min(clip.max.x).ceil() as i32;
    let min_y = p0.y.min(p1.y).min(p2.y).max(clip.min.y).floor() as i32;
    let max_y = p0.y.max(p1.y).max(p2.y).min(clip.max.y).ceil() as i32;
    if min_x >= max_x || min_y >= max_y {
        return;
    }
    let area = edge(p1, p2, p0);
    if area.abs() < 1e-4 {
        return;
    }
    let inv_area = 1.0 / area;
    for py in min_y..max_y {
        if py < 0 || py >= h as i32 {
            continue;
        }
        for px in min_x..max_x {
            if px < 0 || px >= w as i32 {
                continue;
            }
            let p = Px {
                x: px as f32 + 0.5,
                y: py as f32 + 0.5,
            };
            let w0 = edge(p1, p2, p) * inv_area;
            let w1 = edge(p2, p0, p) * inv_area;
            let w2 = edge(p0, p1, p) * inv_area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let uv = egui::pos2(
                v0.uv.x * w0 + v1.uv.x * w1 + v2.uv.x * w2,
                v0.uv.y * w0 + v1.uv.y * w1 + v2.uv.y * w2,
            );
            let mut color = lerp_color(v0.color, v1.color, v2.color, w0, w1, w2);
            if let Some(font) = font_image {
                let cov = sample_font(font, uv.x, uv.y).powf(0.55);
                let a = (cov * color.a() as f32) as u8;
                color = Color32::from_rgba_premultiplied(
                    ((color.r() as f32) * cov) as u8,
                    ((color.g() as f32) * cov) as u8,
                    ((color.b() as f32) * cov) as u8,
                    a,
                );
            }
            let idx = ((py as u32 * w + px as u32) * 4) as usize;
            over_pixel(&mut pixels[idx..idx + 4], color);
        }
    }
}

fn edge(a: Px, b: Px, c: Px) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}

fn lerp_color(c0: Color32, c1: Color32, c2: Color32, w0: f32, w1: f32, w2: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (c0.r() as f32 * w0 + c1.r() as f32 * w1 + c2.r() as f32 * w2) as u8,
        (c0.g() as f32 * w0 + c1.g() as f32 * w1 + c2.g() as f32 * w2) as u8,
        (c0.b() as f32 * w0 + c1.b() as f32 * w1 + c2.b() as f32 * w2) as u8,
        (c0.a() as f32 * w0 + c1.a() as f32 * w1 + c2.a() as f32 * w2) as u8,
    )
}

fn sample_font(font: &egui::FontImage, u: f32, v: f32) -> f32 {
    if u < 0.0 || v < 0.0 || u > 1.0 || v > 1.0 {
        return 0.0;
    }
    let w = font.width().max(1);
    let h = font.height().max(1);
    let x = ((u * w as f32) as usize).min(w - 1);
    let y = ((v * h as f32) as usize).min(h - 1);
    font.pixels[y * w + x]
}

fn rect_to_px(rect: egui::Rect, ppp: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(rect.min.x * ppp, rect.min.y * ppp),
        egui::pos2(rect.max.x * ppp, rect.max.y * ppp),
    )
}

fn over_pixel(dst: &mut [u8], src: Color32) {
    let sa = src.a() as f32 / 255.0;
    if sa <= 0.0 {
        return;
    }
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return;
    }
    let inv_out_a = 1.0 / out_a;
    dst[0] = ((src.r() as f32 + dst[0] as f32 * da * (1.0 - sa)) * inv_out_a) as u8;
    dst[1] = ((src.g() as f32 + dst[1] as f32 * da * (1.0 - sa)) * inv_out_a) as u8;
    dst[2] = ((src.b() as f32 + dst[2] as f32 * da * (1.0 - sa)) * inv_out_a) as u8;
    dst[3] = (out_a * 255.0).round() as u8;
}
