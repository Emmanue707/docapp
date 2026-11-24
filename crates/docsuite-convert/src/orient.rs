use docsuite_core::{CancelToken, Result};
use docsuite_ocr::{GrayImage, Line, Recognizer};

use crate::ocr_util::cap_long_side;

const DETECT_LONG: u32 = 1000;

pub fn line_word_count(lines: &[Line]) -> usize {
    lines.iter().map(|l| l.words.len()).sum()
}

pub fn lines_mean_confidence(lines: &[Line]) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for line in lines {
        for word in &line.words {
            if let Some(c) = word.candidates.first() {
                sum += c.confidence;
                n += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { sum / n as f32 }
}

pub fn ocr_yield_chars(lines: &[Line]) -> usize {
    lines
        .iter()
        .flat_map(|l| &l.words)
        .filter_map(|w| w.candidates.first())
        .map(|c| c.text.chars().count())
        .sum()
}

pub fn needs_orientation_fallback(lines: &[Line]) -> bool {
    let words = line_word_count(lines);
    let chars = ocr_yield_chars(lines);
    let conf = lines_mean_confidence(lines);
    words < 4 || chars < 12 || conf < 0.35
}

pub fn detect_upright_degrees(
    image: &GrayImage,
    recognizer: &dyn Recognizer,
    cancel: &CancelToken,
) -> Result<i32> {
    let (small, _) = cap_long_side(image.clone(), DETECT_LONG);
    let mut best_deg = 0i32;
    let mut best_score = 0usize;
    for deg in [0i32, 90, 180, 270] {
        cancel.check()?;
        let rotated = rotate_gray(&small, deg);
        let lines = recognizer.recognize(&rotated, None, cancel)?;
        let score = line_word_count(&lines);
        if score > best_score {
            best_score = score;
            best_deg = deg;
        }
    }
    Ok(best_deg)
}

fn rotate_gray(image: &GrayImage, degrees: i32) -> GrayImage {
    let deg = degrees.rem_euclid(360);
    if deg == 0 {
        return image.clone();
    }
    let w = image.width;
    let h = image.height;
    let (out_w, out_h) = if deg == 90 || deg == 270 {
        (h, w)
    } else {
        (w, h)
    };
    let mut out = vec![255u8; (out_w * out_h) as usize];
    for y in 0..h {
        for x in 0..w {
            let p = image.get(x, y);
            let (ox, oy) = match deg {
                90 => (h - 1 - y, x),
                180 => (w - 1 - x, h - 1 - y),
                270 => (y, w - 1 - x),
                _ => (x, y),
            };
            out[oy as usize * out_w as usize + ox as usize] = p;
        }
    }
    GrayImage {
        width: out_w,
        height: out_h,
        pixels: out,
    }
}

pub fn rotate_gray_for_ocr(image: &GrayImage, degrees: i32) -> GrayImage {
    rotate_gray(image, degrees)
}

pub fn map_lines_from_rotated(lines: &[Line], degrees: i32, src_w: u32, src_h: u32) -> Vec<Line> {
    if degrees == 0 {
        return lines.to_vec();
    }
    let deg = degrees.rem_euclid(360);
    lines
        .iter()
        .map(|line| {
            let words = line
                .words
                .iter()
                .map(|w| {
                    let bb = map_bbox_inverse(w.bbox, deg, src_w, src_h);
                    docsuite_ocr::Word {
                        bbox: bb,
                        candidates: w.candidates.clone(),
                    }
                })
                .collect();
            let bb = map_bbox_inverse(line.bbox, deg, src_w, src_h);
            Line { bbox: bb, words }
        })
        .collect()
}

fn map_bbox_inverse(
    bb: docsuite_ocr::BBox,
    degrees: i32,
    src_w: u32,
    src_h: u32,
) -> docsuite_ocr::BBox {
    let corners = [
        (bb.x0, bb.y0),
        (bb.x1, bb.y0),
        (bb.x0, bb.y1),
        (bb.x1, bb.y1),
    ];
    let mapped: Vec<(f32, f32)> = corners
        .iter()
        .map(|(x, y)| map_point_inverse(*x, *y, degrees, src_w, src_h))
        .collect();
    let x0 = mapped.iter().map(|(a, _)| *a).fold(f32::INFINITY, f32::min);
    let x1 = mapped
        .iter()
        .map(|(a, _)| *a)
        .fold(f32::NEG_INFINITY, f32::max);
    let y0 = mapped.iter().map(|(_, b)| *b).fold(f32::INFINITY, f32::min);
    let y1 = mapped
        .iter()
        .map(|(_, b)| *b)
        .fold(f32::NEG_INFINITY, f32::max);
    docsuite_ocr::BBox { x0, y0, x1, y1 }
}

fn map_point_inverse(x: f32, y: f32, degrees: i32, src_w: u32, src_h: u32) -> (f32, f32) {
    let deg = degrees.rem_euclid(360);
    match deg {
        90 => (y, src_w as f32 - x),
        180 => (src_w as f32 - x, src_h as f32 - y),
        270 => (src_h as f32 - y, x),
        _ => (x, y),
    }
}
