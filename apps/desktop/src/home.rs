use eframe::egui;

use crate::app::DocApp;
use crate::icon::paint_tool_icon;
use crate::theme::ACCENT;
use crate::tools::{DocTool, home_tools};

impl DocApp {
    pub(crate) fn show_home(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let pad = 12.0;
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                ui.heading(
                    egui::RichText::new("Waghitech Docapp")
                        .size(24.0)
                        .color(ACCENT),
                );
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Open a document or pick a tool below").size(13.5));
            });
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Open file")
                                .size(16.0)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(ACCENT)
                        .min_size(egui::vec2(168.0, 42.0)),
                    )
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("documents", &["pdf", "png", "jpg", "jpeg", "webp"])
                        .pick_file()
                {
                    self.open_path(path);
                }
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Or drop a file here").weak().italics());
            });
            ui.add_space(12.0);
            self.show_tool_grid(ui);
            ui.add_space(12.0);
            self.show_recent_list(ui);
            ui.add_space(pad);
        });
        let _ = ctx;
    }

    fn show_tool_grid(&mut self, ui: &mut egui::Ui) {
        let gap = 10.0;
        let avail = ui.available_width();
        let cols = if avail > 720.0 {
            4
        } else if avail > 480.0 {
            3
        } else {
            2
        };
        let cols = cols.clamp(2, 4);
        let outer_w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
        let cell_w = (outer_w - 24.0).floor().max(100.0);
        let cell_h = 54.0;
        let tools = home_tools();
        let mut index = 0usize;
        while index < tools.len() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                for _ in 0..cols {
                    if index >= tools.len() {
                        break;
                    }
                    let tool = tools[index];
                    let card = egui::Frame::new()
                        .fill(ui.style().visuals.panel_fill)
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            ui.style().visuals.widgets.noninteractive.bg_stroke.color,
                        ))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(cell_w);
                            ui.set_height(cell_h);
                            ui.vertical_centered(|ui| {
                                paint_tool_icon(ui, tool, 28.0);
                                ui.add_space(6.0);
                                ui.add(
                                    egui::Label::new(egui::RichText::new(tool.label()).size(12.5))
                                        .wrap_mode(egui::TextWrapMode::Wrap)
                                        .halign(egui::Align::Center),
                                );
                            });
                        });
                    if card.response.interact(egui::Sense::click()).clicked() {
                        self.start_home_tool(tool);
                    }
                    index += 1;
                }
            });
            if index < tools.len() {
                ui.add_space(gap);
            }
        }
    }

    fn show_recent_list(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label(egui::RichText::new("Recent files").strong());
        ui.add_space(6.0);
        if self.recent.is_empty() {
            ui.label(egui::RichText::new("No recent files").weak());
            return;
        }
        for r in self.recent.clone() {
            let name = r
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_owned();
            let full = r.display().to_string();
            ui.horizontal(|ui| {
                if ui.link(&name).on_hover_text(&full).clicked() {
                    self.open_path(r);
                }
            });
        }
    }

    pub(crate) fn show_result_banner(&mut self, ui: &mut egui::Ui) {
        let Some(banner) = self.result_banner.clone() else {
            return;
        };
        let mut dismiss = false;
        let mut open_in_app: Option<std::path::PathBuf> = None;
        let err_bg = if self.dark_mode {
            egui::Color32::from_rgb(76, 26, 26)
        } else {
            egui::Color32::from_rgb(253, 236, 236)
        };
        let err_fg = if self.dark_mode {
            egui::Color32::from_rgb(255, 195, 195)
        } else {
            egui::Color32::from_rgb(160, 25, 25)
        };
        let fill = if banner.error {
            err_bg
        } else {
            ACCENT.gamma_multiply(0.15)
        };
        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(
                1.0_f32,
                if banner.error { err_fg } else { ACCENT },
            ))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if banner.error {
                        ui.colored_label(err_fg, &banner.message);
                    } else {
                        ui.label(&banner.message);
                    }
                    if let Some(path) = &banner.path {
                        ui.strong(path.display().to_string());
                        if ui.button("Open folder").clicked() {
                            crate::tools::open_parent_dir(path);
                        }
                        if path.extension().and_then(|e| e.to_str()) == Some("pdf")
                            && ui.button("Open in app").clicked()
                        {
                            open_in_app = Some(path.clone());
                        }
                    }
                    if ui.button("Dismiss").clicked() {
                        dismiss = true;
                    }
                });
            });
        if dismiss {
            self.result_banner = None;
        }
        if let Some(path) = open_in_app {
            self.open_path(path);
        }
    }

    pub(crate) fn start_home_tool(&mut self, tool: DocTool) {
        if matches!(tool, DocTool::Ocr) && self.models_missing {
            self.result_banner = Some(crate::app::ResultBanner {
                message: "OCR models not installed — use Models in a document or download below"
                    .to_owned(),
                path: None,
                error: true,
            });
            return;
        }
        let dialog = crate::tools::pick_dialog(tool);
        let files = if tool.multi_select() {
            dialog.pick_files().unwrap_or_default()
        } else {
            dialog.pick_file().into_iter().collect::<Vec<_>>()
        };
        if files.is_empty() {
            return;
        }
        let page_count = files
            .first()
            .and_then(|p| crate::tools::pdf_page_count(p))
            .unwrap_or(0);
        self.tool_modal = Some(crate::app::ToolModal {
            tool,
            inputs: files,
            params: crate::tools::ToolParams::with_page_count(page_count),
        });
    }
}
