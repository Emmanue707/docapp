use std::collections::HashMap;

use docsuite_viewer::{
    PageLayout, Tile, TileKey, Viewer, Viewport, fit_page, page_screen_rect, scale_for_zoom_bucket,
    visible_page_range, visible_tile_keys,
};
use eframe::egui;

pub type TileTexMap = HashMap<TileKey, egui::TextureHandle>;

pub fn thumb_strip_viewport(layout: &PageLayout, width: f32) -> Viewport {
    let max_w = layout
        .page_sizes_pt
        .iter()
        .map(|(w, _)| *w)
        .fold(1.0_f32, f32::max);
    let scale = (width / max_w).max(0.05);
    Viewport {
        width_px: width.max(1.0),
        height_px: (layout.total_height_pt() * scale + 32.0).max(200.0),
        scale,
        offset_x: 0.0,
        offset_y: 0.0,
        dpi: 96.0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paint_page_thumb(
    ctx: &egui::Context,
    viewer: &mut Viewer,
    textures: &mut TileTexMap,
    doc_id: u64,
    strip_vp: Viewport,
    layout: &PageLayout,
    page_idx: usize,
    rect: egui::Rect,
    painter: &egui::Painter,
) {
    let (_, _, sw, sh) = page_screen_rect(strip_vp, layout, page_idx);
    let fit = (rect.width() / sw.max(1.0)).min(rect.height() / sh.max(1.0));
    let draw_w = sw * fit;
    let draw_h = sh * fit;
    let ox = rect.min.x + (rect.width() - draw_w) * 0.5;
    let oy = rect.min.y + (rect.height() - draw_h) * 0.5;
    let page_rect = egui::Rect::from_min_size(egui::pos2(ox, oy), egui::vec2(draw_w, draw_h));
    painter.rect_filled(page_rect, 2.0, egui::Color32::WHITE);
    let bucket_scale =
        scale_for_zoom_bucket(docsuite_viewer::zoom_bucket_for_scale(strip_vp.scale));
    let ratio = (strip_vp.scale / bucket_scale) * fit;
    for key in visible_tile_keys(doc_id, strip_vp, layout, 256) {
        if key.page != page_idx {
            continue;
        }
        let tex = match textures.get(&key) {
            Some(t) => t,
            None => {
                let Some(tile) = viewer.cached_tile(&key) else {
                    continue;
                };
                textures
                    .entry(key)
                    .or_insert_with(|| upload_tile(ctx, key, &tile))
            }
        };
        let [tw, th] = tex.size();
        let x = ox + key.tx as f32 * 256.0 * ratio;
        let y = oy + key.ty as f32 * 256.0 * ratio;
        painter.image(
            tex.id(),
            egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(tw as f32 * ratio, th as f32 * ratio),
            ),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    let _ = fit_page;
}

#[allow(clippy::too_many_arguments)]
pub fn paint_main_viewer(
    ctx: &egui::Context,
    viewer: &mut Viewer,
    textures: &mut TileTexMap,
    doc_id: u64,
    viewport: &Viewport,
    layout: &PageLayout,
    rect: egui::Rect,
    painter: &egui::Painter,
) {
    viewer.update(*viewport);
    let _ = viewer.poll_ready();

    for page in visible_page_range(*viewport, layout) {
        let (sx, sy, sw, sh) = page_screen_rect(*viewport, layout, page);
        let cx = ((rect.width() - sw) * 0.5).max(0.0);
        let page_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + sx + cx, rect.min.y + sy),
            egui::vec2(sw, sh),
        );
        painter.rect_filled(page_rect, 2.0, egui::Color32::WHITE);
        painter.rect_stroke(
            page_rect,
            2.0,
            egui::Stroke::new(1.0_f32, egui::Color32::from_gray(200)),
            egui::StrokeKind::Outside,
        );
    }

    for key in visible_tile_keys(doc_id, *viewport, layout, 256) {
        let tex = match textures.get(&key) {
            Some(t) => t,
            None => {
                let Some(tile) = viewer.cached_tile(&key) else {
                    continue;
                };
                textures
                    .entry(key)
                    .or_insert_with(|| upload_tile(ctx, key, &tile))
            }
        };
        let (sx, sy, sw, _) = page_screen_rect(*viewport, layout, key.page);
        let cx = ((rect.width() - sw) * 0.5).max(0.0);
        let bucket_scale = scale_for_zoom_bucket(key.zoom_bucket);
        let ratio = viewport.scale / bucket_scale;
        let [tw, th] = tex.size();
        let x = rect.min.x + sx + cx + key.tx as f32 * 256.0 * ratio;
        let y = rect.min.y + sy + key.ty as f32 * 256.0 * ratio;
        painter.image(
            tex.id(),
            egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(tw as f32 * ratio, th as f32 * ratio),
            ),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}

fn upload_tile(ctx: &egui::Context, key: TileKey, tile: &Tile) -> egui::TextureHandle {
    let img = egui::ColorImage::from_rgba_unmultiplied(
        [tile.width as usize, tile.height as usize],
        &tile.bytes,
    );
    ctx.load_texture(
        format!(
            "tile_{}_{}_{}_{}_{}",
            key.doc_id, key.page, key.zoom_bucket, key.tx, key.ty
        ),
        img,
        egui::TextureOptions::LINEAR,
    )
}
