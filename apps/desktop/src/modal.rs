use std::path::PathBuf;

use eframe::egui;

use crate::app::DocApp;
use crate::theme::ACCENT;
use crate::tools::{self, DocTool};

impl DocApp {
    pub(crate) fn show_tool_modal(&mut self, ctx: &egui::Context) {
        let Some(mut modal) = self.tool_modal.clone() else {
            return;
        };
        let mut open = true;
        let mut should_close = false;
        let title = modal.tool.label();
        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .default_width(360.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                if modal.tool == DocTool::Merge || modal.tool == DocTool::ImagesToPdf {
                    ui.label("Files:");
                    for p in &modal.inputs {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                        let pages = tools::pdf_page_count(p)
                            .map(|c| format!(" — {c} pages"))
                            .unwrap_or_default();
                        ui.label(format!("{name}{pages}"));
                    }
                    if modal.tool == DocTool::Merge
                        && ui.button("Add PDF…").clicked()
                        && let Some(f) = rfd::FileDialog::new()
                            .add_filter("pdf", &["pdf"])
                            .pick_file()
                    {
                        modal.inputs.push(f);
                    }
                }
                match modal.tool {
                    DocTool::Split => {
                        ui.text_edit_singleline(&mut modal.params.split_ranges);
                        ui.label("ranges (e.g. 1-3;4,6)");
                        ui.text_edit_singleline(&mut modal.params.split_every);
                        ui.label("split every N pages (optional)");
                    }
                    DocTool::Rotate => {
                        ui.text_edit_singleline(&mut modal.params.page_spec);
                        ui.label("page ranges");
                        ui.add(
                            egui::DragValue::new(&mut modal.params.rotate_degrees)
                                .speed(1)
                                .range(-360..=360),
                        );
                    }
                    DocTool::Delete => {
                        ui.text_edit_singleline(&mut modal.params.page_spec);
                        ui.label("pages to remove (ranges)");
                    }
                    DocTool::Crop => {
                        ui.add(egui::DragValue::new(&mut modal.params.crop_page).range(1..=9999));
                        ui.text_edit_singleline(&mut modal.params.crop_rect);
                        ui.label("box x,y,width,height");
                    }
                    DocTool::Reorder => {
                        ui.text_edit_multiline(&mut modal.params.reorder_spec);
                    }
                    DocTool::AddText => {
                        ui.text_edit_singleline(&mut modal.params.stamp_text);
                        ui.add(egui::DragValue::new(&mut modal.params.stamp_page).range(1..=9999));
                    }
                    DocTool::AddImage => {
                        if ui.button("Choose image…").clicked()
                            && let Some(f) = rfd::FileDialog::new()
                                .add_filter("image", &["png", "jpg", "jpeg", "webp"])
                                .pick_file()
                        {
                            modal.params.stamp_image = Some(f);
                        }
                        if let Some(img) = &modal.params.stamp_image {
                            ui.label(img.display().to_string());
                        } else {
                            ui.label("uses a small generated stamp if none chosen");
                        }
                        ui.add(egui::DragValue::new(&mut modal.params.stamp_page).range(1..=9999));
                    }
                    DocTool::Sign => {
                        ui.text_edit_singleline(&mut modal.params.sign_name);
                        ui.text_edit_singleline(&mut modal.params.sign_reason);
                        ui.add(egui::DragValue::new(&mut modal.params.sign_page).range(1..=9999));
                    }
                    DocTool::Ocr => {
                        ui.checkbox(&mut modal.params.handwriting_ocr, "handwriting");
                        if modal.params.handwriting_ocr && self.handwriting_missing {
                            ui.label("handwriting model missing — download from document tools");
                        }
                    }
                    _ => {}
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                    if ui.add(egui::Button::new("Run").fill(ACCENT)).clicked() {
                        if matches!(modal.tool, DocTool::Ocr) && self.models_missing {
                            self.result_banner = Some(crate::app::ResultBanner {
                                message: "download OCR models first".to_owned(),
                                path: None,
                                error: true,
                            });
                            should_close = true;
                        } else if let Some(first) = modal.inputs.first() {
                            let out = pick_output(modal.tool, first, &modal.params);
                            if let Some(output) = out {
                                let tool = modal.tool;
                                let inputs = modal.inputs.clone();
                                let params = modal.params.clone();
                                let models = self.models_dir.clone();
                                self.run_tool_job(tool, inputs, output, params, models);
                                should_close = true;
                            }
                        }
                    }
                });
            });
        if should_close {
            open = false;
        }
        if open {
            self.tool_modal = Some(modal);
        } else {
            self.tool_modal = None;
        }
    }

    pub(crate) fn show_in_doc_tools_window(&mut self, ctx: &egui::Context) {
        let mut open = self.in_doc_tools;
        if !open {
            return;
        }
        egui::Window::new("Document tools")
            .open(&mut open)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Crop mode").clicked() {
                        self.interaction = crate::state::ViewerInteractionMode::Crop;
                        self.in_doc_tools = false;
                    }
                    if ui.button("Sign (drag)").clicked() {
                        self.interaction = crate::state::ViewerInteractionMode::Signature;
                        self.in_doc_tools = false;
                    }
                    if ui.button("Normal view").clicked() {
                        self.interaction = crate::state::ViewerInteractionMode::Normal;
                    }
                });
                ui.separator();
                if ui.button("Split…").clicked() {
                    self.in_doc_tools = false;
                    if let Some(path) = self.path.clone() {
                        let pc = self.pdf.as_ref().map(|p| p.page_count()).unwrap_or(0);
                        self.tool_modal = Some(crate::app::ToolModal {
                            tool: DocTool::Split,
                            inputs: vec![path],
                            params: tools::ToolParams::with_page_count(pc),
                        });
                    }
                }
                if self.models_missing {
                    if ui.button("Download OCR models").clicked() {
                        let dir = self.models_dir.clone();
                        self.run_bg("models", move |_c, _p| {
                            docsuite_models::fetch_models(&dir).map_err(|e| e.to_string())
                        });
                    }
                } else if ui.button("Make searchable").clicked() {
                    self.in_doc_tools = false;
                    if let Some(path) = self.path.clone() {
                        self.tool_modal = Some(crate::app::ToolModal {
                            tool: DocTool::Ocr,
                            inputs: vec![path],
                            params: tools::ToolParams::default(),
                        });
                    }
                }
            });
        self.in_doc_tools = open;
    }
}

fn pick_output(
    tool: DocTool,
    input: &std::path::Path,
    params: &tools::ToolParams,
) -> Option<PathBuf> {
    if tool == DocTool::PdfToImages {
        return rfd::FileDialog::new().pick_folder();
    }
    if tool == DocTool::Split && !params.split_every.trim().is_empty() {
        return rfd::FileDialog::new().pick_folder();
    }
    let (label, ext) = tools::save_filter(tool);
    let name = tools::default_save_name(tool, input);
    let mut d = rfd::FileDialog::new().set_file_name(&name);
    if !ext.is_empty() {
        d = d.add_filter(label, ext);
    }
    d.save_file()
}
