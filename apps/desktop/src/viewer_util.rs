use docsuite_viewer::{PageLayout, Viewer, Viewport, fit_page};
use eframe::egui;

pub fn thumb_viewport_for_page(
    layout: &PageLayout,
    page_idx: usize,
    width: f32,
    height: f32,
) -> Viewport {
    let base = Viewport {
        width_px: width,
        height_px: height,
        scale: 0.2,
        offset_x: 0.0,
        offset_y: 0.0,
        dpi: 96.0,
    };
    fit_page(base, layout, page_idx)
}

pub fn paint_tiles_in_rect(
    ctx: &egui::Context,
    viewer: &mut Viewer,
    viewport: Viewport,
    layout: &PageLayout,
    rect: egui::Rect,
    painter: &egui::Painter,
) {
    viewer.update(viewport);
    let ready = viewer.poll_ready();
    for rt in ready {
        if let Some(tex) = tile_texture(ctx, &rt.tile) {
            let (sx, sy, _, _) = docsuite_viewer::page_screen_rect(viewport, layout, rt.key.page);
            let ts = 256.0;
            let x = rect.min.x + sx + rt.key.tx as f32 * ts - viewport.offset_x;
            let y = rect.min.y + sy + rt.key.ty as f32 * ts - viewport.offset_y;
            painter.image(
                tex.id(),
                egui::Rect::from_min_size(
                    egui::pos2(x, y),
                    egui::vec2(rt.tile.width as f32, rt.tile.height as f32),
                ),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }
}

pub fn paint_main_viewer(
    ctx: &egui::Context,
    viewer: &mut Viewer,
    viewport: &Viewport,
    layout: &PageLayout,
    rect: egui::Rect,
    painter: &egui::Painter,
) {
    paint_tiles_in_rect(ctx, viewer, *viewport, layout, rect, painter);
}

fn tile_texture(ctx: &egui::Context, tile: &docsuite_viewer::Tile) -> Option<egui::TextureHandle> {
    let img = egui::ColorImage::from_rgba_unmultiplied(
        [tile.width as usize, tile.height as usize],
        &tile.bytes,
    );
    Some(ctx.load_texture(
        format!("tile_{}_{}_{}", tile.width, tile.height, tile.bytes.len()),
        img,
        egui::TextureOptions::LINEAR,
    ))
}
