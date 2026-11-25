use eframe::egui;

use crate::app::DocApp;
use crate::theme::ACCENT;
use crate::viewer_util::{paint_page_thumb, thumb_strip_viewport};

impl DocApp {
    pub(crate) fn show_page_sidebar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Pages");
        ui.add_space(4.0);
        let page_count = self.layout.page_sizes_pt.len();
        if page_count == 0 {
            return;
        }
        if self.thumb_order.len() != page_count {
            self.thumb_order = (1..=page_count as u32).collect();
        }
        let strip_vp = thumb_strip_viewport(&self.layout, 94.0);
        if let Some(tv) = self.thumb_viewer.as_mut() {
            tv.update(strip_vp);
            let _ = tv.poll_ready();
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut i = 0usize;
            while i < self.thumb_order.len() {
                let page = self.thumb_order[i];
                let page_idx = page.saturating_sub(1) as usize;
                let selected = self.selection.selected_vec().contains(&page);
                let thumb_h = 104.0;
                ui.horizontal(|ui| {
                    let frame = egui::Frame::new()
                        .fill(if selected {
                            ACCENT.gamma_multiply(0.2)
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::new(
                            if selected { 2.0_f32 } else { 1.0_f32 },
                            if selected {
                                ACCENT
                            } else {
                                ui.style().visuals.widgets.noninteractive.bg_stroke.color
                            },
                        ))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::same(4));
                    let inner = frame.show(ui, |ui| {
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(94.0, thumb_h),
                            egui::Sense::click_and_drag(),
                        );
                        if let Some(tv) = self.thumb_viewer.as_mut() {
                            let painter = ui.painter_at(rect);
                            paint_page_thumb(
                                ctx,
                                tv,
                                &mut self.thumb_textures,
                                2,
                                strip_vp,
                                &self.layout,
                                page_idx,
                                rect,
                                &painter,
                            );
                        }
                        ui.label(format!("Page {page}"));
                        resp
                    });
                    let resp = inner.inner;
                    if resp.clicked() {
                        let extend = ui.input(|i| i.modifiers.shift);
                        self.selection.toggle(page, extend, page_count as u32);
                        self.jump_to_page(page);
                    }
                    if resp.drag_started() {
                        self.thumb_drag = Some(i);
                    }
                    if resp.hovered()
                        && ui.input(|i| i.pointer.any_released())
                        && let Some(from) = self.thumb_drag.take()
                        && from != i
                    {
                        self.selection
                            .apply_reorder_drag(from, i, &mut self.thumb_order);
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        if arrow_button(ui, true, i > 0).clicked() && i > 0 {
                            self.selection
                                .apply_reorder_drag(i, i - 1, &mut self.thumb_order);
                        }
                        if arrow_button(ui, false, i + 1 < self.thumb_order.len()).clicked()
                            && i + 1 < self.thumb_order.len()
                        {
                            self.selection
                                .apply_reorder_drag(i, i + 1, &mut self.thumb_order);
                        }
                    });
                });
                i += 1;
            }
        });
        ui.separator();
        let order = self.thumb_order.clone();
        if ui.button("Apply page order").clicked() {
            self.apply_page_order(&order);
        }
        ui.horizontal(|ui| {
            if ui.button("Rotate left").clicked() {
                self.rotate_selected(-90);
            }
            if ui.button("Rotate right").clicked() {
                self.rotate_selected(90);
            }
        });
        if ui.button("Delete selected").clicked() {
            self.delete_selected();
        }
    }

    fn jump_to_page(&mut self, page: u32) {
        let idx = page.saturating_sub(1) as usize;
        if idx >= self.layout.page_sizes_pt.len() {
            return;
        }
        self.viewport = docsuite_viewer::fit_page(self.viewport, &self.layout, idx);
        self.current_page = idx;
    }
}

fn arrow_button(ui: &mut egui::Ui, up: bool, enabled: bool) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), sense);
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&resp);
        let fill = if enabled {
            visuals.bg_fill
        } else {
            ui.style().visuals.widgets.noninteractive.bg_fill
        };
        ui.painter()
            .rect(rect, 4.0, fill, visuals.bg_stroke, egui::StrokeKind::Inside);
        let c = rect.center();
        let color = if enabled {
            visuals.text_color()
        } else {
            ui.style().visuals.weak_text_color()
        };
        let d = 4.0;
        let pts = if up {
            vec![
                egui::pos2(c.x, c.y - d),
                egui::pos2(c.x + d, c.y + d * 0.7),
                egui::pos2(c.x - d, c.y + d * 0.7),
            ]
        } else {
            vec![
                egui::pos2(c.x - d, c.y - d * 0.7),
                egui::pos2(c.x + d, c.y - d * 0.7),
                egui::pos2(c.x, c.y + d),
            ]
        };
        ui.painter()
            .add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
    }
    resp
}
