use eframe::egui;

use crate::app::DocApp;
use crate::icon::paint_tool_icon;
use crate::theme::ACCENT;
use crate::tools::{DocTool, home_tools};

impl DocApp {
    pub(crate) fn show_home(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let pad = 28.0;
        ui.add_space(pad);
        ui.vertical_centered(|ui| {
            ui.heading(
                egui::RichText::new("Waghitech Docapp")
                    .size(26.0)
                    .color(ACCENT),
            );
            ui.add_space(6.0);
            ui.label("Open a document or pick a tool below");
            ui.add_space(16.0);
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("  Open file…  ").size(16.0))
                        .fill(ACCENT)
                        .min_size(egui::vec2(160.0, 40.0)),
                )
                .clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("documents", &["pdf", "png", "jpg", "jpeg", "webp"])
                    .pick_file()
            {
                self.open_path(path);
            }
        });
        ui.add_space(pad);
        self.show_tool_grid(ui);
        ui.add_space(12.0);
        self.show_recent_list(ui);
        ui.add_space(pad);
        let _ = ctx;
    }

    fn show_tool_grid(&mut self, ui: &mut egui::Ui) {
        let cols = if ui.available_width() > 720.0 {
            4
        } else if ui.available_width() > 480.0 {
            3
        } else {
            2
        };
        egui::Grid::new("home_tools")
            .num_columns(cols)
            .spacing(egui::vec2(14.0, 14.0))
            .show(ui, |ui| {
                for (i, &tool) in home_tools().iter().enumerate() {
                    let size = egui::vec2(148.0, 88.0);
                    let frame = egui::Frame::new()
                        .fill(ui.style().visuals.panel_fill)
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            ui.style().visuals.widgets.noninteractive.bg_stroke.color,
                        ))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(10));
                    let card = frame.show(ui, |ui| {
                        ui.set_min_size(size);
                        ui.vertical_centered(|ui| {
                            paint_tool_icon(ui, tool, 28.0);
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(tool.label()).size(12.5));
                        });
                    });
                    if card.response.interact(egui::Sense::click()).clicked() {
                        self.start_home_tool(tool);
                    }
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    fn show_recent_list(&mut self, ui: &mut egui::Ui) {
        if self.recent.is_empty() {
            return;
        }
        ui.separator();
        ui.label(egui::RichText::new("Recent").strong());
        ui.add_space(4.0);
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
        let fill = if banner.error {
            egui::Color32::from_rgb(120, 40, 40)
        } else {
            ACCENT.gamma_multiply(0.15)
        };
        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(
                1.0_f32,
                if banner.error {
                    egui::Color32::RED
                } else {
                    ACCENT
                },
            ))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if banner.error {
                        ui.colored_label(egui::Color32::RED, &banner.message);
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
