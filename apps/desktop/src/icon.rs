use eframe::egui;

pub fn window_icon() -> egui::IconData {
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let in_letter = letter_pixel(x, y);
            if in_letter {
                rgba.extend_from_slice(&[30, 120, 220, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    egui::IconData {
        rgba,
        width: 64,
        height: 64,
    }
}

fn letter_pixel(x: u32, y: u32) -> bool {
    let pad = 14u32;
    if x < pad || y < pad || x >= 64 - pad || y >= 64 - pad {
        return false;
    }
    let lx = x - pad;
    let ly = y - pad;
    let w = 64 - 2 * pad;
    let h = 64 - 2 * pad;
    let stroke = 5u32;
    lx < stroke
        || (ly < stroke && lx < w / 2 + stroke)
        || (ly > h - stroke && lx > w / 3)
        || (ly > h / 2 - stroke && ly < h / 2 + stroke && lx < w * 2 / 3)
}
