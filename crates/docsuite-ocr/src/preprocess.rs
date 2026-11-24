use crate::gray::GrayImage;
use crate::types::BBox;

pub fn rgba_to_grayscale(width: u32, height: u32, rgba: &[u8]) -> Option<GrayImage> {
    let n = (width as usize) * (height as usize);
    if rgba.len() < n * 4 {
        return None;
    }
    let mut pixels = Vec::with_capacity(n);
    for i in 0..n {
        let o = i * 4;
        let r = rgba[o] as u32;
        let g = rgba[o + 1] as u32;
        let b = rgba[o + 2] as u32;
        let y = (299 * r + 587 * g + 114 * b) / 1000;
        pixels.push(y as u8);
    }
    GrayImage::new(width, height, pixels)
}

pub fn otsu_threshold(hist: &[u32; 256]) -> u8 {
    let total: u64 = hist.iter().map(|&h| h as u64).sum();
    if total == 0 {
        return 128;
    }
    let mut sum: u64 = 0;
    for (i, &h) in hist.iter().enumerate() {
        sum += i as u64 * h as u64;
    }
    let mut sum_b = 0u64;
    let mut w_b = 0u64;
    let mut max_var = 0.0f64;
    let mut threshold = 0u8;
    for (t, &count) in hist.iter().enumerate() {
        w_b += count as u64;
        if w_b == 0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0 {
            break;
        }
        sum_b += t as u64 * count as u64;
        let m_b = sum_b as f64 / w_b as f64;
        let m_f = (sum - sum_b) as f64 / w_f as f64;
        let var = w_b as f64 * w_f as f64 * (m_b - m_f).powi(2);
        if var > max_var {
            max_var = var;
            threshold = t as u8;
        }
    }
    threshold
}

pub fn binarize_otsu(image: &GrayImage) -> GrayImage {
    let mut hist = [0u32; 256];
    for &p in &image.pixels {
        hist[p as usize] += 1;
    }
    let t = otsu_threshold(&hist);
    let pixels: Vec<u8> = image
        .pixels
        .iter()
        .map(|&p| if p > t { 255 } else { 0 })
        .collect();
    GrayImage {
        width: image.width,
        height: image.height,
        pixels,
    }
}

pub fn horizontal_projection_lines(image: &GrayImage, min_gap: u32) -> Vec<BBox> {
    let mut rows = vec![0u32; image.height as usize];
    for y in 0..image.height {
        let mut dark = 0u32;
        for x in 0..image.width {
            if image.get(x, y) < 128 {
                dark += 1;
            }
        }
        rows[y as usize] = dark;
    }
    let mut lines = Vec::new();
    let mut y = 0u32;
    while y < image.height {
        while y < image.height && rows[y as usize] == 0 {
            y += 1;
        }
        if y >= image.height {
            break;
        }
        let y0 = y;
        while y < image.height && rows[y as usize] > 0 {
            y += 1;
        }
        let y1 = y;
        if y1 > y0 {
            lines.push(BBox {
                x0: 0.0,
                y0: y0 as f32,
                x1: image.width as f32,
                y1: y1 as f32,
            });
        }
        y += min_gap.max(1);
    }
    lines
}

pub const MAX_OCR_LONG_SIDE: u32 = 3000;

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

pub fn crop_gray(image: &GrayImage, region: BBox) -> GrayImage {
    let x0 = region.x0.floor().max(0.0) as u32;
    let y0 = region.y0.floor().max(0.0) as u32;
    let x1 = region.x1.ceil().min(image.width as f32) as u32;
    let y1 = region.y1.ceil().min(image.height as f32) as u32;
    let w = x1.saturating_sub(x0).max(1);
    let h = y1.saturating_sub(y0).max(1);
    let mut pixels = vec![255u8; (w * h) as usize];
    for row in 0..h {
        for col in 0..w {
            pixels[row as usize * w as usize + col as usize] = image.get(x0 + col, y0 + row);
        }
    }
    GrayImage {
        width: w,
        height: h,
        pixels,
    }
}

pub fn estimate_deskew_degrees(image: &GrayImage) -> f32 {
    let mut sum = 0.0f64;
    let mut count = 0u64;
    for y in 1..image.height.saturating_sub(1) {
        let mut left = 0u32;
        let mut right = 0u32;
        for x in 0..image.width / 2 {
            if image.get(x, y) < 128 {
                left = x;
                break;
            }
        }
        for x in (image.width / 2..image.width).rev() {
            if image.get(x, y) < 128 {
                right = x;
                break;
            }
        }
        if right > left {
            let mid = (left + right) as f64 * 0.5;
            let dx = mid - image.width as f64 * 0.5;
            sum += dx / image.height as f64;
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    (sum / count as f64 * 5.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otsu_splits_bimodal_histogram() {
        let mut hist = [0u32; 256];
        for v in hist.iter_mut().take(50) {
            *v = 100;
        }
        for v in hist.iter_mut().skip(200) {
            *v = 100;
        }
        let t = otsu_threshold(&hist);
        assert!((49..=201).contains(&t));
    }

    #[test]
    fn line_segmentation_finds_three_bars() {
        let w = 100u32;
        let h = 80u32;
        let mut pixels = vec![255u8; (w * h) as usize];
        for bar in [10u32, 35, 60] {
            for y in bar..bar + 8 {
                for x in 10..90 {
                    pixels[y as usize * w as usize + x as usize] = 0;
                }
            }
        }
        let img = GrayImage::new(w, h, pixels).expect("img");
        let lines = horizontal_projection_lines(&img, 2);
        assert_eq!(lines.len(), 3);
    }
}
