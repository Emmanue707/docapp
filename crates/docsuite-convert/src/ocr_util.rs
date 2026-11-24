use docsuite_core::{Error, Result};
use docsuite_ocr::rgba_to_grayscale;
use docsuite_ocr::{BBox, GrayImage, Line};
use image::GenericImageView;

use crate::types::{PlacedImage, RgbColor, TextLine, TextRun};

pub const MAX_OCR_LONG_SIDE: u32 = 2200;

pub fn decode_image_gray(bytes: &[u8]) -> Result<GrayImage> {
    let img = image::load_from_memory(bytes).map_err(|e| Error::Ocr(e.to_string()))?;
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    rgba_to_grayscale(w, h, rgba.as_raw()).ok_or_else(|| Error::Ocr("bad image size".to_owned()))
}

pub fn cap_long_side(image: GrayImage, max_long: u32) -> (GrayImage, f64) {
    let long = image.width.max(image.height);
    if long <= max_long {
        return (image, 1.0);
    }
    let scale = max_long as f64 / long as f64;
    let nw = ((image.width as f64) * scale).round().max(1.0) as u32;
    let nh = ((image.height as f64) * scale).round().max(1.0) as u32;
    let mut out = vec![255u8; (nw * nh) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let sx = ((x as f64) / scale).floor() as u32;
            let sy = ((y as f64) / scale).floor() as u32;
            out[y as usize * nw as usize + x as usize] =
                image.get(sx.min(image.width - 1), sy.min(image.height - 1));
        }
    }
    (
        GrayImage {
            width: nw,
            height: nh,
            pixels: out,
        },
        scale,
    )
}

#[allow(dead_code)]
pub fn region_for_viewport(region: Option<BBox>, width: u32, height: u32) -> Option<BBox> {
    let r = region?;
    let x0 = r.x0.max(0.0);
    let y0 = r.y0.max(0.0);
    let x1 = r.x1.min(width as f32);
    let y1 = r.y1.min(height as f32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(BBox { x0, y0, x1, y1 })
}

pub fn lines_to_text_runs(
    lines: &[Line],
    img: &PlacedImage,
    ocr_w: u32,
    ocr_h: u32,
    highlight_low: bool,
) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let sx = if ocr_w > 0 {
        img.width / ocr_w as f64
    } else {
        1.0
    };
    let sy = if ocr_h > 0 {
        img.height / ocr_h as f64
    } else {
        1.0
    };
    for line in lines {
        for word in &line.words {
            let text = word
                .candidates
                .first()
                .map(|c| c.text.as_str())
                .unwrap_or("");
            if text.is_empty() {
                continue;
            }
            let conf = word.candidates.first().map(|c| c.confidence).unwrap_or(1.0);
            let bb = &word.bbox;
            let x = img.x + bb.x0 as f64 * sx;
            let y = img.y + bb.y0 as f64 * sy;
            let w = (bb.x1 - bb.x0).max(1.0) as f64 * sx;
            let h = (bb.y1 - bb.y0).max(1.0) as f64 * sy;
            let font_size = h * 0.85;
            runs.push(TextRun {
                text: text.to_owned(),
                x,
                y,
                width: w,
                font_size,
                font_key: String::new(),
                font_family: "Arial".to_owned(),
                bold: false,
                italic: false,
                color: RgbColor { r: 0, g: 0, b: 0 },
                char_spacing: 0.0,
                hidden: false,
                confidence: conf,
                highlight: highlight_low && conf < 0.6,
            });
        }
    }
    runs
}

pub fn runs_to_text_lines(runs: &[TextRun]) -> Vec<TextLine> {
    if runs.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<&TextRun> = runs.iter().collect();
    sorted.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    let line_tol = sorted.iter().map(|r| r.font_size).fold(12.0f64, f64::max) * 0.6;
    let mut out = Vec::new();
    let mut current: Vec<TextRun> = Vec::new();
    let mut line_y = sorted[0].y;
    let mut line_h = sorted[0].font_size;
    for run in sorted {
        if (run.y - line_y).abs() > line_tol && !current.is_empty() {
            out.push(finish_line(&current, line_y, line_h));
            current.clear();
            line_y = run.y;
            line_h = run.font_size;
        }
        current.push(run.clone());
        line_h = line_h.max(run.font_size);
    }
    if !current.is_empty() {
        out.push(finish_line(&current, line_y, line_h));
    }
    out
}

fn finish_line(runs: &[TextRun], y: f64, height: f64) -> TextLine {
    let x = runs.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
    let width = runs
        .iter()
        .map(|r| r.x + r.width - x)
        .fold(0.0f64, f64::max);
    TextLine {
        runs: runs.to_vec(),
        x,
        y,
        width,
        height,
    }
}

pub fn apply_ocr_to_scanned_page(
    page: &mut crate::types::ExtractedPage,
    runs: Vec<TextRun>,
    behind_image: bool,
) {
    if behind_image {
        for img in &mut page.images {
            img.behind = true;
        }
    }
    page.text = runs_to_text_lines(&runs);
}

pub fn lines_plain_text(lines: &[Line]) -> String {
    let mut out = String::new();
    for line in lines {
        let mut first = true;
        for word in &line.words {
            let t = word
                .candidates
                .first()
                .map(|c| c.text.as_str())
                .unwrap_or("");
            if t.is_empty() {
                continue;
            }
            if !first {
                out.push(' ');
            }
            first = false;
            out.push_str(t);
        }
        if !first {
            out.push('\n');
        }
    }
    out
}
