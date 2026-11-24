use std::io::Cursor;

use docsuite_core::{Error, Result};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, GenericImageView};

use crate::matrix::Matrix;
use crate::types::{
    ExtractedPage, ImageKind, PathSegment, PlacedImage, TextRun, VectorPath,
};

pub fn normalize_rotate(rotate: i32) -> i32 {
    rotate.rem_euclid(360)
}

pub fn page_size_display(box4: [f64; 4], rotate: i32) -> (f64, f64) {
    let w = (box4[2] - box4[0]).abs();
    let h = (box4[3] - box4[1]).abs();
    match normalize_rotate(rotate) {
        90 | 270 => (h, w),
        _ => (w, h),
    }
}

pub fn page_size_user(box4: [f64; 4]) -> (f64, f64) {
    ((box4[2] - box4[0]).abs(), (box4[3] - box4[1]).abs())
}

fn user_pdf_point(px: f64, py: f64, box4: [f64; 4]) -> (f64, f64) {
    let uy = box4[3] - py;
    let ux = box4[0] + px;
    (ux, uy)
}

pub fn user_top_left_to_display(px: f64, py: f64, box4: [f64; 4], rotate: i32) -> (f64, f64) {
    let (ux, uy) = user_pdf_point(px, py, box4);
    let llx = box4[0];
    let lly = box4[1];
    let urx = box4[2];
    let ury = box4[3];
    match normalize_rotate(rotate) {
        0 => (ux - llx, ury - uy),
        90 => (uy - lly, urx - ux),
        180 => (urx - ux, uy - lly),
        270 => (ury - uy, ux - llx),
        _ => (ux - llx, ury - uy),
    }
}

pub fn display_top_left_to_user(dx: f64, dy: f64, box4: [f64; 4], rotate: i32) -> (f64, f64) {
    let llx = box4[0];
    let lly = box4[1];
    let urx = box4[2];
    let ury = box4[3];
    let (ux, uy) = match normalize_rotate(rotate) {
        0 => (llx + dx, ury - dy),
        90 => (urx - dy, lly + dx),
        180 => (urx - dx, lly + dy),
        270 => (llx + dy, ury - dx),
        _ => (llx + dx, ury - dy),
    };
    let px = ux - llx;
    let py = ury - uy;
    (px, py)
}

fn transform_bbox(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    box4: [f64; 4],
    rotate: i32,
) -> (f64, f64, f64, f64) {
    let corners = [
        user_top_left_to_display(x, y, box4, rotate),
        user_top_left_to_display(x + w, y, box4, rotate),
        user_top_left_to_display(x, y + h, box4, rotate),
        user_top_left_to_display(x + w, y + h, box4, rotate),
    ];
    let min_x = corners
        .iter()
        .map(|(a, _)| *a)
        .fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|(a, _)| *a)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners
        .iter()
        .map(|(_, b)| *b)
        .fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|(_, b)| *b)
        .fold(f64::NEG_INFINITY, f64::max);
    (
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

fn image_rotation_quadrant(m: Matrix) -> i32 {
    let (a, b, c, d) = (m.0[0], m.0[1], m.0[2], m.0[3]);
    let eps = 1e-3;
    if b.abs() < eps && c.abs() < eps {
        if a.signum() * d.signum() < 0.0 {
            return 180;
        }
        return 0;
    }
    if a.abs() < eps && d.abs() < eps {
        if b > 0.0 && c < 0.0 {
            return 90;
        }
        if b < 0.0 && c > 0.0 {
            return 270;
        }
    }
    0
}

pub fn rotate_image_bytes(
    bytes: &[u8],
    kind: ImageKind,
    degrees: i32,
) -> Result<(Vec<u8>, ImageKind)> {
    let deg = normalize_rotate(degrees);
    if deg == 0 {
        return Ok((bytes.to_vec(), kind));
    }
    let img = image::load_from_memory(bytes).map_err(|e| Error::Pdf(e.to_string()))?;
    let rotated = match deg {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    };
    encode_dynamic(rotated, kind)
}

fn encode_dynamic(img: DynamicImage, kind: ImageKind) -> Result<(Vec<u8>, ImageKind)> {
    let mut buf = Vec::new();
    match kind {
        ImageKind::Jpeg => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut enc = JpegEncoder::new_with_quality(&mut buf, 85);
            enc.encode(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .map_err(|e| Error::Pdf(e.to_string()))?;
            Ok((buf, ImageKind::Jpeg))
        }
        ImageKind::Png => {
            img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
                .map_err(|e| Error::Pdf(e.to_string()))?;
            Ok((buf, ImageKind::Png))
        }
    }
}

fn covers_page(img: &PlacedImage, page_w: f64, page_h: f64) -> bool {
    let area = img.width * img.height;
    let page_area = page_w * page_h;
    page_area > 0.0 && area / page_area > 0.8
}

pub fn apply_display_rotation(
    page: &mut ExtractedPage,
    box4: [f64; 4],
    rotate: i32,
    image_ctm: Option<Matrix>,
) {
    let rot = normalize_rotate(rotate);
    let (user_w, user_h) = page_size_user(box4);
    let (disp_w, disp_h) = page_size_display(box4, rotate);
    page.width = disp_w;
    page.height = disp_h;
    page.source_rotate = rot;
    page.crop_box = box4;

    let extra = image_ctm.map(image_rotation_quadrant).unwrap_or(0);
    let mut pixel_rot = rot;
    if rot == 0 && extra != 0 {
        pixel_rot = extra;
    }

    for img in &mut page.images {
        let full = covers_page(img, user_w, user_h);
        let (x, y, w, h) = transform_bbox(img.x, img.y, img.width, img.height, box4, rot);
        img.x = x;
        img.y = y;
        img.width = w;
        img.height = h;
        if full && pixel_rot != 0 {
            if let Ok((bytes, kind)) = rotate_image_bytes(&img.bytes, img.kind, pixel_rot) {
                img.bytes = bytes;
                img.kind = kind;
                img.x = 0.0;
                img.y = 0.0;
                img.width = disp_w;
                img.height = disp_h;
                img.orient_fallback = false;
            }
        }
    }

    for line in &mut page.text {
        for run in &mut line.runs {
            let (x, y, w, h) = transform_bbox(run.x, run.y, run.width, run.font_size, box4, rot);
            run.x = x;
            run.y = y;
            run.width = w;
            run.font_size = run.font_size.max(h * 0.5);
        }
    }
    for line in &mut page.text {
        let (x, y, w, h) = transform_bbox(line.x, line.y, line.width, line.height, box4, rot);
        line.x = x;
        line.y = y;
        line.width = w;
        line.height = h;
    }

    for vp in &mut page.vectors {
        for seg in &mut vp.segments {
            match seg {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => {
                    let (dx, dy) = user_top_left_to_display(p.x, p.y, box4, rot);
                    p.x = dx;
                    p.y = dy;
                }
                PathSegment::CurveTo(a, b, c) => {
                    for p in [a, b, c] {
                        let (dx, dy) = user_top_left_to_display(p.x, p.y, box4, rot);
                        p.x = dx;
                        p.y = dy;
                    }
                }
                PathSegment::Close => {}
            }
        }
    }
    let _ = (user_w, user_h);
}

pub fn display_run_to_user_space(run: &TextRun, page: &ExtractedPage) -> TextRun {
    let mut out = run.clone();
    let (px, py) = display_top_left_to_user(run.x, run.y, page.crop_box, page.source_rotate);
    let (px2, py2) = display_top_left_to_user(
        run.x + run.width,
        run.y + run.font_size,
        page.crop_box,
        page.source_rotate,
    );
    out.x = px;
    out.y = py;
    out.width = (px2 - px).abs().max(1.0);
    let _ = py2;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_270_maps_full_page() {
        let box4 = [0.0, 0.0, 841.68, 595.2];
        let (x, y) = user_top_left_to_display(0.0, 0.0, box4, 270);
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
        let (x2, y2) = user_top_left_to_display(841.68, 595.2, box4, 270);
        assert!((x2 - 595.2).abs() < 0.1);
        assert!((y2 - 841.68).abs() < 0.1);
    }

    #[test]
    fn unit_ctm_extent_not_pixels() {
        let m = Matrix([841.68, 0.0, 0.0, 595.2, 0.0, 0.0]);
        let corners = [
            m.transform(0.0, 0.0),
            m.transform(1.0, 0.0),
            m.transform(0.0, 1.0),
            m.transform(1.0, 1.0),
        ];
        let w = (corners[1].0 - corners[0].0).abs();
        let h = (corners[2].1 - corners[0].1).abs();
        assert!((w - 841.68).abs() < 0.01);
        assert!((h - 595.2).abs() < 0.01);
    }
}
