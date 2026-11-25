use crate::tile::{TileKey, TileRectPx, scale_for_zoom_bucket, zoom_bucket_for_scale};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width_px: f32,
    pub height_px: f32,
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub dpi: f32,
}

#[derive(Clone, Debug)]
pub struct PageLayout {
    pub page_sizes_pt: Vec<(f32, f32)>,
    pub gap_pt: f32,
}

impl PageLayout {
    pub fn page_top_y_pt(&self, page: usize) -> f32 {
        let mut y = 0.0f32;
        for (i, (_, h)) in self.page_sizes_pt.iter().enumerate() {
            if i == page {
                break;
            }
            y += h + self.gap_pt;
        }
        y
    }

    pub fn total_height_pt(&self) -> f32 {
        if self.page_sizes_pt.is_empty() {
            return 0.0;
        }
        let pages: f32 = self.page_sizes_pt.iter().map(|(_, h)| h).sum();
        let gaps = self.gap_pt * (self.page_sizes_pt.len().saturating_sub(1)) as f32;
        pages + gaps
    }

    pub fn page_at_doc_y_pt(&self, mut y_pt: f32) -> Option<usize> {
        for (i, (_, h)) in self.page_sizes_pt.iter().enumerate() {
            if y_pt < *h {
                return Some(i);
            }
            y_pt -= h + self.gap_pt;
            if y_pt < 0.0 {
                return Some(i + 1).filter(|&p| p < self.page_sizes_pt.len());
            }
        }
        self.page_sizes_pt.len().checked_sub(1)
    }
}

pub fn zoom_around_point(vp: Viewport, new_scale: f32, focal_x: f32, focal_y: f32) -> Viewport {
    let new_scale = new_scale.max(1e-4);
    if vp.scale <= 0.0 {
        return Viewport {
            scale: new_scale,
            ..vp
        };
    }
    let doc_x = (focal_x + vp.offset_x) / vp.scale;
    let doc_y = (focal_y + vp.offset_y) / vp.scale;
    Viewport {
        scale: new_scale,
        offset_x: doc_x * new_scale - focal_x,
        offset_y: doc_y * new_scale - focal_y,
        ..vp
    }
}

pub fn pan(vp: Viewport, dx: f32, dy: f32, layout: &PageLayout) -> Viewport {
    let mut out = vp;
    out.offset_x += dx;
    out.offset_y += dy;
    clamp_pan(&mut out, layout);
    out
}

pub fn clamp_pan(vp: &mut Viewport, layout: &PageLayout) {
    let doc_h = layout.total_height_pt() * vp.scale;
    let doc_w = max_page_width_pt(layout) * vp.scale;
    let min_off_x = vp.width_px - doc_w;
    let max_off_x = 0.0f32;
    if doc_w <= vp.width_px {
        vp.offset_x = (vp.width_px - doc_w) * 0.5;
    } else {
        vp.offset_x = vp.offset_x.clamp(min_off_x, max_off_x);
    }
    let min_off_y = vp.height_px - doc_h;
    let max_off_y = 0.0f32;
    if doc_h <= vp.height_px {
        vp.offset_y = (vp.height_px - doc_h) * 0.5;
    } else {
        vp.offset_y = vp.offset_y.clamp(min_off_y, max_off_y);
    }
}

fn max_page_width_pt(layout: &PageLayout) -> f32 {
    layout
        .page_sizes_pt
        .iter()
        .map(|(w, _)| *w)
        .fold(0.0f32, f32::max)
}

pub fn fit_width(vp: Viewport, layout: &PageLayout) -> Viewport {
    let w = max_page_width_pt(layout);
    if w <= 0.0 || vp.width_px <= 0.0 {
        return vp;
    }
    let scale = vp.width_px / w;
    let doc_h = layout.total_height_pt() * scale;
    Viewport {
        scale,
        offset_x: 0.0,
        offset_y: if doc_h <= vp.height_px {
            (vp.height_px - doc_h) * 0.5
        } else {
            0.0
        },
        ..vp
    }
}

pub fn fit_page(vp: Viewport, layout: &PageLayout, page: usize) -> Viewport {
    let Some(&(w, h)) = layout.page_sizes_pt.get(page) else {
        return vp;
    };
    if w <= 0.0 || h <= 0.0 || vp.width_px <= 0.0 || vp.height_px <= 0.0 {
        return vp;
    }
    let scale = (vp.width_px / w).min(vp.height_px / h);
    let top = layout.page_top_y_pt(page);
    Viewport {
        scale,
        offset_x: 0.0,
        offset_y: top * scale,
        ..vp
    }
}

pub fn visible_page_range(vp: Viewport, layout: &PageLayout) -> std::ops::Range<usize> {
    let top_pt = vp.offset_y / vp.scale.max(1e-6);
    let bottom_pt = (vp.offset_y + vp.height_px) / vp.scale.max(1e-6);
    let start = layout.page_at_doc_y_pt(top_pt).unwrap_or(0);
    let mut end = layout
        .page_at_doc_y_pt(bottom_pt)
        .unwrap_or_else(|| layout.page_sizes_pt.len().saturating_sub(1));
    if end < start {
        end = start;
    }
    start..(end + 1).min(layout.page_sizes_pt.len())
}

pub fn page_screen_rect(vp: Viewport, layout: &PageLayout, page: usize) -> (f32, f32, f32, f32) {
    let top_pt = layout.page_top_y_pt(page);
    let (w_pt, h_pt) = layout.page_sizes_pt[page];
    let x = -vp.offset_x;
    let y = top_pt * vp.scale - vp.offset_y;
    (x, y, w_pt * vp.scale, h_pt * vp.scale)
}

pub fn visible_tile_keys(
    doc_id: u64,
    vp: Viewport,
    layout: &PageLayout,
    tile_size: u32,
) -> Vec<TileKey> {
    let bucket = zoom_bucket_for_scale(vp.scale);
    let bucket_scale = scale_for_zoom_bucket(bucket);
    let mut keys = Vec::new();
    for page in visible_page_range(vp, layout) {
        let (sx, sy, sw, sh) = page_screen_rect(vp, layout, page);
        if sw <= 0.0 || sh <= 0.0 {
            continue;
        }
        let page_w_px = layout.page_sizes_pt[page].0 * bucket_scale;
        let page_h_px = layout.page_sizes_pt[page].1 * bucket_scale;
        let ts = tile_size as f32;
        let cols = ((page_w_px / ts).ceil() as i32).max(1);
        let rows = ((page_h_px / ts).ceil() as i32).max(1);
        let vis_left = (-sx).max(0.0);
        let vis_top = (-sy).max(0.0);
        let vis_right = (vp.width_px - sx).min(sw);
        let vis_bottom = (vp.height_px - sy).min(sh);
        if vis_right <= vis_left || vis_bottom <= vis_top {
            continue;
        }
        let to_bucket = bucket_scale / vp.scale.max(1e-6);
        let tx0 = ((vis_left * to_bucket) / ts).floor() as i32;
        let ty0 = ((vis_top * to_bucket) / ts).floor() as i32;
        let tx1 = (((vis_right * to_bucket) / ts).ceil() as i32).min(cols);
        let ty1 = (((vis_bottom * to_bucket) / ts).ceil() as i32).min(rows);
        for ty in ty0..ty1 {
            for tx in tx0..tx1 {
                keys.push(TileKey {
                    doc_id,
                    page,
                    zoom_bucket: bucket,
                    tx,
                    ty,
                });
            }
        }
    }
    keys
}

pub fn tile_rect_px_for_key(layout: &PageLayout, key: TileKey, tile_size: u32) -> TileRectPx {
    let scale = scale_for_zoom_bucket(key.zoom_bucket);
    let (pw, ph) = layout.page_sizes_pt[key.page];
    let page_w = pw * scale;
    let page_h = ph * scale;
    let ts = tile_size as f32;
    let x = key.tx as f32 * ts;
    let y = key.ty as f32 * ts;
    let w = ts.min(page_w - x).max(0.0);
    let h = ts.min(page_h - y).max(0.0);
    TileRectPx {
        x,
        y,
        width: w,
        height: h,
    }
}

pub fn prefetch_tile_keys(
    doc_id: u64,
    vp: Viewport,
    layout: &PageLayout,
    tile_size: u32,
) -> Vec<TileKey> {
    let mut keys = visible_tile_keys(doc_id, vp, layout, tile_size);
    let range = visible_page_range(vp, layout);
    let bucket = zoom_bucket_for_scale(vp.scale);
    let ts = tile_size;
    if range.start > 0 {
        keys.extend(ring_for_page(doc_id, range.start - 1, bucket, layout, ts));
    }
    if range.end < layout.page_sizes_pt.len() {
        keys.extend(ring_for_page(doc_id, range.end, bucket, layout, ts));
    }
    let ring = viewport_ring_keys(doc_id, vp, layout, tile_size, bucket);
    for k in ring {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys
}

fn ring_for_page(
    doc_id: u64,
    page: usize,
    bucket: u32,
    layout: &PageLayout,
    tile_size: u32,
) -> Vec<TileKey> {
    let scale = scale_for_zoom_bucket(bucket);
    let (pw, ph) = layout.page_sizes_pt[page];
    let page_w_px = pw * scale;
    let page_h_px = ph * scale;
    let ts = tile_size as f32;
    let cols = ((page_w_px / ts).ceil() as i32).max(1);
    let rows = ((page_h_px / ts).ceil() as i32).max(1);
    let mut out = Vec::new();
    for ty in 0..rows {
        for tx in 0..cols {
            out.push(TileKey {
                doc_id,
                page,
                zoom_bucket: bucket,
                tx,
                ty,
            });
        }
    }
    out
}

fn viewport_ring_keys(
    doc_id: u64,
    vp: Viewport,
    layout: &PageLayout,
    tile_size: u32,
    bucket: u32,
) -> Vec<TileKey> {
    let pad = tile_size as f32;
    let ring_vp = Viewport {
        offset_x: vp.offset_x - pad,
        offset_y: vp.offset_y - pad,
        width_px: vp.width_px + 2.0 * pad,
        height_px: vp.height_px + 2.0 * pad,
        ..vp
    };
    let mut ring = visible_tile_keys(doc_id, ring_vp, layout, tile_size);
    let vis = visible_tile_keys(doc_id, vp, layout, tile_size);
    ring.retain(|k| !vis.contains(k));
    let _ = bucket;
    ring
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_layout() -> PageLayout {
        PageLayout {
            page_sizes_pt: vec![(612.0, 792.0), (612.0, 792.0)],
            gap_pt: 12.0,
        }
    }

    #[test]
    fn zoom_keeps_focal_point_fixed() {
        let vp = Viewport {
            width_px: 800.0,
            height_px: 600.0,
            scale: 1.0,
            offset_x: 10.0,
            offset_y: 20.0,
            dpi: 72.0,
        };
        let next = zoom_around_point(vp, 2.0, 100.0, 200.0);
        let doc_x = (100.0 + vp.offset_x) / vp.scale;
        let doc_y = (200.0 + vp.offset_y) / vp.scale;
        assert!((next.offset_x + 100.0 - doc_x * next.scale).abs() < 1e-3);
        assert!((next.offset_y + 200.0 - doc_y * next.scale).abs() < 1e-3);
    }

    #[test]
    fn visible_tiles_cover_viewport() {
        let layout = sample_layout();
        let vp = Viewport {
            width_px: 512.0,
            height_px: 512.0,
            scale: 1.5,
            offset_x: 0.0,
            offset_y: 0.0,
            dpi: 72.0,
        };
        let keys = visible_tile_keys(1, vp, &layout, 256);
        assert!(!keys.is_empty());
        assert!(
            keys.iter()
                .all(|k| k.zoom_bucket == zoom_bucket_for_scale(vp.scale))
        );
    }

    #[test]
    fn visible_tiles_when_scrolled_uses_bucket_coords() {
        let layout = sample_layout();
        let vp = Viewport {
            width_px: 512.0,
            height_px: 512.0,
            scale: 1.5,
            offset_x: 0.0,
            offset_y: 1300.0,
            dpi: 72.0,
        };
        let keys = visible_tile_keys(1, vp, &layout, 256);
        assert!(keys.iter().any(|k| k.page == 1 && k.ty == 0));
    }

    #[test]
    fn bucket_reuse_while_zooming() {
        let a = zoom_bucket_for_scale(1.4);
        let b = zoom_bucket_for_scale(1.45);
        assert_eq!(a, b);
        let c = zoom_bucket_for_scale(2.0);
        assert_ne!(a, c);
    }

    #[test]
    fn pan_clamps_when_document_smaller_than_view() {
        let layout = sample_layout();
        let vp = Viewport {
            width_px: 2000.0,
            height_px: 2000.0,
            scale: 0.5,
            offset_x: -500.0,
            offset_y: -500.0,
            dpi: 72.0,
        };
        let panned = pan(vp, 100.0, 100.0, &layout);
        assert!(panned.offset_x >= 0.0);
    }
}
