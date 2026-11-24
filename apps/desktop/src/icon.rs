use eframe::egui;

use crate::theme::ACCENT;

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

pub fn paint_tool_icon(
    ui: &mut egui::Ui,
    tool: crate::tools::DocTool,
    size: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let c = ACCENT;
    let s = rect.shrink(2.0);
    let stroke = egui::Stroke::new(1.8_f32, c);
    match tool {
        crate::tools::DocTool::PdfToWord => {
            let w = s.width() * 0.45;
            let doc = egui::Rect::from_min_size(s.min, egui::vec2(w, s.height()));
            painter.rect_stroke(doc, 2.0, stroke, egui::StrokeKind::Outside);
            let arr = s.max.x - w * 0.35;
            painter.line_segment(
                [
                    egui::pos2(arr, s.center().y - 6.0),
                    egui::pos2(s.max.x - 2.0, s.center().y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(arr, s.center().y + 6.0),
                    egui::pos2(s.max.x - 2.0, s.center().y),
                ],
                stroke,
            );
        }
        crate::tools::DocTool::Merge => {
            let w = s.width() * 0.38;
            for (i, dx) in [0.0, w * 0.55].iter().enumerate() {
                let r = egui::Rect::from_min_size(
                    egui::pos2(s.min.x + dx, s.min.y + i as f32 * 3.0),
                    egui::vec2(w, s.height() - 4.0),
                );
                painter.rect_stroke(r, 1.5, stroke, egui::StrokeKind::Outside);
            }
        }
        crate::tools::DocTool::Split => {
            let mid = s.center().x;
            let doc = s.shrink2(egui::vec2(4.0, 2.0));
            painter.rect_stroke(doc, 2.0, stroke, egui::StrokeKind::Outside);
            painter.line_segment(
                [egui::pos2(mid, doc.min.y), egui::pos2(mid, doc.max.y)],
                stroke,
            );
        }
        crate::tools::DocTool::Rotate => {
            let center = s.center();
            let r = s.width() * 0.35;
            painter.circle_stroke(center, r, stroke);
            painter.line_segment(
                [center, egui::pos2(center.x + r * 0.7, center.y - r * 0.5)],
                stroke,
            );
        }
        crate::tools::DocTool::Delete => {
            let body = egui::Rect::from_center_size(
                egui::pos2(s.center().x, s.center().y + 4.0),
                egui::vec2(s.width() * 0.55, s.height() * 0.45),
            );
            painter.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Outside);
            painter.line_segment(
                [
                    egui::pos2(s.min.x + 4.0, body.min.y - 2.0),
                    egui::pos2(s.max.x - 4.0, body.min.y - 2.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(s.center().x - 3.0, body.center().y - 3.0),
                    egui::pos2(s.center().x + 3.0, body.center().y + 3.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(s.center().x + 3.0, body.center().y - 3.0),
                    egui::pos2(s.center().x - 3.0, body.center().y + 3.0),
                ],
                stroke,
            );
        }
        crate::tools::DocTool::Reorder => {
            let bar_w = s.width() * 0.22;
            for (i, y_off) in [0.0f32, s.height() * 0.32, s.height() * 0.64]
                .iter()
                .enumerate()
            {
                let r = egui::Rect::from_min_size(
                    egui::pos2(s.min.x + i as f32 * bar_w * 0.4, s.min.y + y_off),
                    egui::vec2(bar_w, s.height() * 0.22),
                );
                painter.rect_filled(r, 1.0, c.gamma_multiply(0.25 + i as f32 * 0.15));
                painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Outside);
            }
        }
        crate::tools::DocTool::Crop => {
            let inner = s.shrink(5.0);
            painter.rect_stroke(
                s,
                1.5,
                egui::Stroke::new(1.5_f32, stroke.color.gamma_multiply(0.4)),
                egui::StrokeKind::Outside,
            );
            painter.rect_stroke(inner, 1.5, stroke, egui::StrokeKind::Outside);
            for corner in [
                inner.min,
                egui::pos2(inner.max.x, inner.min.y),
                inner.max,
                egui::pos2(inner.min.x, inner.max.y),
            ] {
                painter.circle_filled(corner, 2.5, c);
            }
        }
        crate::tools::DocTool::AddText => {
            painter.line_segment(
                [
                    egui::pos2(s.min.x + 3.0, s.max.y - 4.0),
                    egui::pos2(s.max.x - 3.0, s.max.y - 4.0),
                ],
                stroke,
            );
            painter.text(
                s.center() + egui::vec2(0.0, -2.0),
                egui::Align2::CENTER_CENTER,
                "T",
                egui::FontId::proportional(s.height() * 0.55),
                c,
            );
        }
        crate::tools::DocTool::AddImage => {
            let frame = s.shrink(3.0);
            painter.rect_stroke(frame, 1.5, stroke, egui::StrokeKind::Outside);
            let sun = egui::pos2(frame.max.x - 6.0, frame.min.y + 6.0);
            painter.circle_filled(sun, 3.0, c);
            painter.line_segment(
                [
                    egui::pos2(frame.min.x + 3.0, frame.max.y - 4.0),
                    frame.center(),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    frame.center(),
                    egui::pos2(frame.max.x - 3.0, frame.max.y - 4.0),
                ],
                stroke,
            );
        }
        crate::tools::DocTool::Sign => {
            painter.line_segment(
                [
                    egui::pos2(s.min.x + 2.0, s.max.y - 5.0),
                    egui::pos2(s.max.x - 2.0, s.max.y - 5.0),
                ],
                stroke,
            );
            let pts = [
                egui::pos2(s.min.x + 4.0, s.max.y - 6.0),
                egui::pos2(s.min.x + 12.0, s.center().y),
                egui::pos2(s.center().x, s.max.y - 8.0),
                egui::pos2(s.max.x - 6.0, s.min.y + 8.0),
            ];
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], stroke);
            }
        }
        crate::tools::DocTool::Ocr => {
            let eye = s.shrink(6.0);
            painter.circle_stroke(eye.center(), eye.width() * 0.45, stroke);
            painter.circle_filled(eye.center(), 3.0, c);
        }
        crate::tools::DocTool::ImagesToPdf => {
            for (i, dx) in [0.0, 4.0].iter().enumerate() {
                let r = egui::Rect::from_min_size(
                    egui::pos2(s.min.x + dx, s.min.y + (1 - i) as f32 * 4.0),
                    egui::vec2(s.width() * 0.5, s.height() * 0.55),
                );
                painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Outside);
            }
            let doc = egui::Rect::from_min_size(
                egui::pos2(s.min.x + s.width() * 0.35, s.min.y + s.height() * 0.35),
                egui::vec2(s.width() * 0.55, s.height() * 0.55),
            );
            painter.rect_stroke(doc, 1.5, stroke, egui::StrokeKind::Outside);
        }
        crate::tools::DocTool::OdtToWord | crate::tools::DocTool::WordToOdt => {
            let left = egui::Rect::from_min_size(s.min, egui::vec2(s.width() * 0.42, s.height()));
            let right = egui::Rect::from_min_size(
                egui::pos2(s.min.x + s.width() * 0.52, s.min.y),
                egui::vec2(s.width() * 0.42, s.height()),
            );
            painter.rect_stroke(left, 1.5, stroke, egui::StrokeKind::Outside);
            painter.rect_stroke(right, 1.5, stroke, egui::StrokeKind::Outside);
            painter.line_segment(
                [
                    egui::pos2(s.center().x - 2.0, s.center().y - 4.0),
                    egui::pos2(s.center().x + 6.0, s.center().y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(s.center().x - 2.0, s.center().y + 4.0),
                    egui::pos2(s.center().x + 6.0, s.center().y),
                ],
                stroke,
            );
        }
        crate::tools::DocTool::ExtractText => {
            for (i, y) in [0.15, 0.4, 0.65].iter().enumerate() {
                let w = s.width() * (0.9 - i as f32 * 0.15);
                let r = egui::Rect::from_min_size(
                    egui::pos2(s.min.x + 2.0, s.min.y + s.height() * y),
                    egui::vec2(w, 2.5),
                );
                painter.rect_filled(r, 0.0, c.gamma_multiply(0.5 + i as f32 * 0.15));
            }
        }
        crate::tools::DocTool::PdfToImages => {
            let doc = egui::Rect::from_min_size(s.min, egui::vec2(s.width() * 0.45, s.height()));
            painter.rect_stroke(doc, 1.5, stroke, egui::StrokeKind::Outside);
            for (i, dy) in [0.0, 5.0].iter().enumerate() {
                let img = egui::Rect::from_min_size(
                    egui::pos2(s.min.x + s.width() * 0.42 + i as f32 * 4.0, s.min.y + dy),
                    egui::vec2(s.width() * 0.45, s.height() * 0.42),
                );
                painter.rect_stroke(img, 1.0, stroke, egui::StrokeKind::Outside);
            }
        }
    }
    response
}
