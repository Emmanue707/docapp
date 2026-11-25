use std::path::PathBuf;
use std::sync::Arc;

use docsuite_core::{CancelToken, Error, JobHandle, JobRunner, Progress, ProgressSink};
use docsuite_pdf::{Pdf, PdfRect, SignatureStamp, visible_signature};
use docsuite_viewer::{
    DEFAULT_CACHE_DESKTOP, ImageBackend, PageLayout, PdfiumBackend, RenderBackend, Viewer,
    Viewport, fit_page, fit_width, pan, visible_page_range, zoom_around_point,
};
use eframe::egui;

use crate::recent::{load_recent, push_recent};
use crate::state::{
    PageSelection, ScreenPoint, UndoStack, ViewerInteractionMode, screen_drag_to_pdf_rect,
};
use crate::theme::{self, ACCENT};
use crate::tools::{self, DocTool, ToolParams};
use crate::viewer_util::paint_main_viewer;

const UNDO_CAP: usize = 20;

#[derive(Clone, Debug)]
pub struct ResultBanner {
    pub message: String,
    pub path: Option<PathBuf>,
    pub error: bool,
}

#[derive(Clone, Debug)]
pub struct ToolModal {
    pub tool: DocTool,
    pub inputs: Vec<PathBuf>,
    pub params: ToolParams,
}

pub struct DocApp {
    pub(crate) runner: JobRunner,
    progress: Arc<std::sync::Mutex<Option<Progress>>>,
    cancel: CancelToken,
    job: Option<JobHandle<()>>,
    pdf_bytes: Option<Vec<u8>>,
    pub(crate) pdf: Option<Pdf>,
    pub(crate) path: Option<PathBuf>,
    dirty: bool,
    undo: UndoStack,
    pub(crate) selection: PageSelection,
    pub(crate) viewer: Option<Viewer>,
    pub(crate) layout: PageLayout,
    pub(crate) viewport: Viewport,
    pub(crate) current_page: usize,
    search_hits: Vec<(u32, PdfRect)>,
    pub(crate) models_dir: PathBuf,
    pub(crate) models_missing: bool,
    pub(crate) handwriting_missing: bool,
    pub(crate) recent: Vec<PathBuf>,
    panning: bool,
    space_pan: bool,
    crop_drag: Option<(ScreenPoint, ScreenPoint, u32)>,
    signature_drag: Option<(ScreenPoint, ScreenPoint, u32)>,
    pub(crate) interaction: ViewerInteractionMode,
    pub(crate) dark_mode: bool,
    pub(crate) tool_modal: Option<ToolModal>,
    pub(crate) result_banner: Option<ResultBanner>,
    pub(crate) thumb_order: Vec<u32>,
    pub(crate) thumb_drag: Option<usize>,
    pub(crate) in_doc_tools: bool,
    home_active: bool,
    pending_drop_tool: Option<DocTool>,
    pending_tool_result: Option<PathBuf>,
    pub(crate) screenshot_path: Option<PathBuf>,
    screenshot_frames: u32,
}

impl Default for DocApp {
    fn default() -> Self {
        let models_dir = docsuite_models::default_dir();
        let st = docsuite_models::status(&models_dir);
        let models_missing = !st.ready();
        let handwriting_missing = !st.handwriting;
        Self {
            runner: JobRunner::builder().workers(2).build(),
            progress: Arc::new(std::sync::Mutex::new(None)),
            cancel: CancelToken::new(),
            job: None,
            pdf_bytes: None,
            pdf: None,
            path: None,
            dirty: false,
            undo: UndoStack::new(UNDO_CAP),
            selection: PageSelection::default(),
            viewer: None,
            layout: PageLayout {
                page_sizes_pt: Vec::new(),
                gap_pt: 12.0,
            },
            viewport: Viewport {
                width_px: 800.0,
                height_px: 600.0,
                scale: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,
                dpi: 96.0,
            },
            current_page: 0,
            search_hits: Vec::new(),
            models_dir,
            models_missing,
            handwriting_missing,
            recent: load_recent(),
            panning: false,
            space_pan: false,
            crop_drag: None,
            signature_drag: None,
            interaction: ViewerInteractionMode::Normal,
            dark_mode: false,
            tool_modal: None,
            result_banner: None,
            thumb_order: Vec::new(),
            thumb_drag: None,
            in_doc_tools: false,
            home_active: true,
            pending_drop_tool: None,
            pending_tool_result: None,
            screenshot_path: None,
            screenshot_frames: 0,
        }
    }
}

impl DocApp {
    fn sync_models(&mut self) {
        let st = docsuite_models::status(&self.models_dir);
        self.models_missing = !st.ready();
        self.handwriting_missing = !st.handwriting;
    }

    pub(crate) fn open_path(&mut self, path: PathBuf) {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.result_banner = Some(ResultBanner {
                    message: format!("open failed: {e}"),
                    path: None,
                    error: true,
                });
                return;
            }
        };
        if bytes.starts_with(b"%PDF") {
            match Pdf::load_bytes(&bytes) {
                Ok(pdf) => self.load_pdf(path, bytes, pdf),
                Err(e) => {
                    self.result_banner = Some(ResultBanner {
                        message: format!("{e}"),
                        path: None,
                        error: true,
                    });
                }
            }
        } else if image::guess_format(&bytes).is_ok() {
            self.load_image(path, bytes);
        } else {
            self.result_banner = Some(ResultBanner {
                message: "unsupported file type".to_owned(),
                path: None,
                error: true,
            });
        }
    }

    fn load_pdf(&mut self, path: PathBuf, bytes: Vec<u8>, pdf: Pdf) {
        let backend: Arc<dyn docsuite_viewer::RenderBackend> =
            match PdfiumBackend::from_bytes(bytes.clone()) {
                Ok(b) => Arc::new(b),
                Err(e) => {
                    self.result_banner = Some(ResultBanner {
                        message: format!("{e}"),
                        path: None,
                        error: true,
                    });
                    return;
                }
            };
        let mut sizes = Vec::new();
        for pn in 1..=pdf.page_count() {
            if let Ok((w, h)) = backend.page_size((pn - 1) as usize) {
                sizes.push((w, h));
            }
        }
        let layout = PageLayout {
            page_sizes_pt: sizes,
            gap_pt: 12.0,
        };
        let viewer = Viewer::with_runner(
            1,
            backend,
            layout.clone(),
            DEFAULT_CACHE_DESKTOP,
            256,
            JobRunner::builder().workers(2).build(),
        );
        self.pdf_bytes = Some(bytes);
        self.pdf = Some(pdf);
        self.path = Some(path.clone());
        self.dirty = false;
        self.undo.clear();
        self.selection.clear();
        self.layout = layout;
        self.viewer = Some(viewer);
        self.viewport = fit_width(self.viewport, &self.layout);
        self.current_page = 0;
        self.search_hits.clear();
        self.thumb_order = (1..=self.layout.page_sizes_pt.len() as u32).collect();
        self.home_active = false;
        self.interaction = ViewerInteractionMode::Normal;
        push_recent(&path);
        self.recent = load_recent();
        self.result_banner = None;
    }

    fn load_image(&mut self, path: PathBuf, bytes: Vec<u8>) {
        let backend = match ImageBackend::from_bytes(&bytes, 96.0) {
            Ok(b) => Arc::new(b),
            Err(e) => {
                self.result_banner = Some(ResultBanner {
                    message: format!("{e}"),
                    path: None,
                    error: true,
                });
                return;
            }
        };
        let (w, h) = RenderBackend::page_size(backend.as_ref(), 0).unwrap_or((612.0, 792.0));
        let layout = PageLayout {
            page_sizes_pt: vec![(w, h)],
            gap_pt: 0.0,
        };
        let viewer = Viewer::with_runner(
            1,
            backend,
            layout.clone(),
            DEFAULT_CACHE_DESKTOP,
            256,
            JobRunner::builder().workers(2).build(),
        );
        self.pdf_bytes = None;
        self.pdf = None;
        self.path = Some(path.clone());
        self.dirty = false;
        self.undo.clear();
        self.layout = layout;
        self.viewer = Some(viewer);
        self.viewport = fit_width(self.viewport, &self.layout);
        self.thumb_order = vec![1];
        self.home_active = false;
        push_recent(&path);
        self.recent = load_recent();
    }

    fn go_home(&mut self) {
        self.home_active = true;
        self.viewer = None;
        self.pdf = None;
        self.pdf_bytes = None;
        self.path = None;
        self.dirty = false;
        self.selection.clear();
        self.layout.page_sizes_pt.clear();
        self.interaction = ViewerInteractionMode::Normal;
        self.crop_drag = None;
        self.signature_drag = None;
    }

    fn checkpoint(&mut self) {
        if let Some(bytes) = &self.pdf_bytes {
            self.undo.push_checkpoint(bytes);
        }
    }

    fn replace_pdf_bytes(&mut self, bytes: Vec<u8>) {
        self.checkpoint();
        match Pdf::load_bytes(&bytes) {
            Ok(pdf) => {
                if let Some(path) = self.path.clone() {
                    self.load_pdf(path, bytes, pdf);
                }
                self.dirty = true;
            }
            Err(e) => {
                self.result_banner = Some(ResultBanner {
                    message: format!("{e}"),
                    path: None,
                    error: true,
                });
            }
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        let Some(pdf) = self.pdf.as_mut() else {
            self.result_banner = Some(ResultBanner {
                message: "nothing to save".to_owned(),
                path: None,
                error: true,
            });
            return;
        };
        match pdf.save(&path) {
            Ok(()) => {
                self.path = Some(path.clone());
                self.dirty = false;
                if let Ok(b) = std::fs::read(&path) {
                    self.pdf_bytes = Some(b);
                }
                push_recent(&path);
                self.recent = load_recent();
                self.result_banner = Some(ResultBanner {
                    message: "saved".to_owned(),
                    path: Some(path),
                    error: false,
                });
            }
            Err(e) => {
                self.result_banner = Some(ResultBanner {
                    message: format!("save failed: {e}"),
                    path: None,
                    error: true,
                });
            }
        }
    }

    pub(crate) fn run_bg<F>(&mut self, stage: &'static str, work: F)
    where
        F: FnOnce(CancelToken, Option<ProgressSink>) -> Result<(), String> + Send + 'static,
    {
        if self.job.is_some() {
            return;
        }
        self.cancel = CancelToken::new();
        let cancel = self.cancel.clone();
        let progress_slot = Arc::clone(&self.progress);
        let sink: ProgressSink = Arc::new(move |p: Progress| {
            if let Ok(mut slot) = progress_slot.lock() {
                *slot = Some(p);
            }
        });
        let handle = self
            .runner
            .submit(docsuite_core::Priority::Normal, move |_| {
                work(cancel, Some(sink)).map_err(|s| Error::Backend(s.to_owned()))
            });
        self.job = Some(handle);
        let _ = stage;
    }

    pub(crate) fn run_tool_job(
        &mut self,
        tool: DocTool,
        inputs: Vec<PathBuf>,
        output: PathBuf,
        params: ToolParams,
        models_dir: PathBuf,
    ) {
        self.pending_tool_result = Some(output.clone());
        self.run_bg("tool", move |cancel, progress| {
            tools::run_tool(
                tool,
                &inputs,
                &output,
                &params,
                &models_dir,
                &cancel,
                &progress,
            )?;
            Ok(())
        });
    }
}

impl DocApp {
    fn poll_job(&mut self) {
        let Some(handle) = &self.job else {
            return;
        };
        if let Some(res) = handle.try_result() {
            self.job = None;
            if let Ok(mut slot) = self.progress.lock() {
                *slot = None;
            }
            match res {
                Ok(()) => {
                    if let Some(path) = self.pending_tool_result.take() {
                        self.result_banner = Some(ResultBanner {
                            message: "done —".to_owned(),
                            path: Some(path),
                            error: false,
                        });
                    } else {
                        self.result_banner = Some(ResultBanner {
                            message: "done".to_owned(),
                            path: None,
                            error: false,
                        });
                    }
                }
                Err(Error::Cancelled) => {
                    self.pending_tool_result = None;
                    self.result_banner = Some(ResultBanner {
                        message: "cancelled".to_owned(),
                        path: None,
                        error: true,
                    });
                }
                Err(e) => {
                    self.pending_tool_result = None;
                    self.result_banner = Some(ResultBanner {
                        message: format!("{e}"),
                        path: None,
                        error: true,
                    });
                }
            }
        }
    }

    pub(crate) fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.dark_mode = !self.dark_mode;
        theme::apply_visuals(ctx, self.dark_mode);
    }
}

impl eframe::App for DocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job();
        self.sync_models();
        theme::apply_visuals(ctx, self.dark_mode);

        if ctx.input(|i| i.viewport().close_requested()) && self.dirty {
            self.result_banner = Some(ResultBanner {
                message: "unsaved changes".to_owned(),
                path: None,
                error: true,
            });
        }

        let progress_snapshot = self.progress.lock().ok().and_then(|g| g.as_ref().cloned());
        if let Some(progress) = progress_snapshot {
            egui::TopBottomPanel::bottom("progress").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let frac = progress.done as f32 / progress.total.max(1) as f32;
                    ui.add(egui::ProgressBar::new(frac).show_percentage());
                    ui.label(format!(
                        "{} {}/{}",
                        progress.stage, progress.done, progress.total
                    ));
                    if ui.button("Cancel").clicked() {
                        self.cancel.cancel();
                    }
                });
            });
        } else if self.job.is_some() {
            egui::TopBottomPanel::bottom("progress").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Working…");
                    if ui.button("Cancel").clicked() {
                        self.cancel.cancel();
                    }
                });
            });
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Home").clicked() {
                    self.go_home();
                }
                if let Some(path) = &self.path {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("document");
                    ui.strong(name);
                    if self.dirty {
                        ui.label(egui::RichText::new("•").color(ACCENT));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Theme").clicked() {
                        self.toggle_theme(ctx);
                    }
                    if self.viewer.is_some() {
                        if ui.button("Tools").clicked() {
                            self.in_doc_tools = true;
                        }
                        if ui
                            .add_enabled(self.undo.can_undo(), egui::Button::new("Undo"))
                            .clicked()
                            && let Some(cur) = self.pdf_bytes.as_ref()
                            && let Some(bytes) = self.undo.undo(cur)
                            && let Ok(pdf) = Pdf::load_bytes(&bytes)
                            && let Some(path) = self.path.clone()
                        {
                            self.load_pdf(path, bytes, pdf);
                        }
                        if ui
                            .add_enabled(self.pdf.is_some(), egui::Button::new("Save"))
                            .clicked()
                            && let Some(path) = self.path.clone()
                        {
                            self.save_to(path);
                        }
                        if ui.button("Fit page").clicked() {
                            self.viewport =
                                fit_page(self.viewport, &self.layout, self.current_page);
                        }
                        if ui.button("Fit width").clicked() {
                            self.viewport = fit_width(self.viewport, &self.layout);
                        }
                        if ui.button("+").clicked() {
                            let focal = egui::pos2(
                                self.viewport.width_px * 0.5,
                                self.viewport.height_px * 0.5,
                            );
                            self.viewport = zoom_around_point(
                                self.viewport,
                                self.viewport.scale * 1.2,
                                focal.x,
                                focal.y,
                            );
                        }
                        if ui.button("−").clicked() {
                            let focal = egui::pos2(
                                self.viewport.width_px * 0.5,
                                self.viewport.height_px * 0.5,
                            );
                            self.viewport = zoom_around_point(
                                self.viewport,
                                self.viewport.scale / 1.2,
                                focal.x,
                                focal.y,
                            );
                        }
                    }
                });
            });
        });

        if self.home_active || self.viewer.is_none() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().inner_margin(egui::Margin::same(20)))
                .show(ctx, |ui| {
                    self.show_result_banner(ui);
                    self.show_home(ctx, ui);
                });
        } else {
            egui::SidePanel::left("thumbs")
                .resizable(true)
                .default_width(132.0)
                .show(ctx, |ui| {
                    self.show_page_sidebar(ctx, ui);
                });
            egui::CentralPanel::default().show(ctx, |ui| {
                self.show_result_banner(ui);
                self.show_viewer(ui, ctx);
            });
        }

        self.show_tool_modal(ctx);
        self.show_in_doc_tools_window(ctx);

        ctx.input(|i| {
            if i.modifiers.command
                && i.key_pressed(egui::Key::O)
                && let Some(path) = rfd::FileDialog::new().pick_file()
            {
                self.open_path(path);
            }
        });

        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            for f in ctx.input(|i| i.raw.dropped_files.clone()) {
                if let Some(path) = f.path {
                    if self.home_active {
                        self.open_path(path);
                    } else if let Some(tool) = self.pending_drop_tool {
                        let pc = tools::pdf_page_count(&path).unwrap_or(0);
                        self.tool_modal = Some(ToolModal {
                            tool,
                            inputs: vec![path],
                            params: ToolParams::with_page_count(pc),
                        });
                        self.pending_drop_tool = None;
                    } else {
                        self.open_path(path);
                    }
                }
            }
        }

        if self.screenshot_path.is_some() {
            self.screenshot_frames = self.screenshot_frames.saturating_add(1);
            if self.screenshot_frames >= 2
                && let Some(path) = self.screenshot_path.take()
            {
                match crate::screenshot::write_cpu_png(ctx, &path) {
                    Ok(()) => std::process::exit(0),
                    Err(e) => {
                        eprintln!("screenshot: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }

        ctx.request_repaint();
    }
}

impl DocApp {
    fn show_viewer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.max_rect();
        self.viewport.width_px = rect.width();
        self.viewport.height_px = rect.height();
        let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let scroll = ui.input(|i| i.raw_scroll_delta.y + i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let mods = ui.input(|i| i.modifiers.command);
            if mods {
                let pos = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
                let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
                self.viewport = zoom_around_point(
                    self.viewport,
                    self.viewport.scale * factor,
                    pos.x - rect.min.x,
                    pos.y - rect.min.y,
                );
            } else {
                self.viewport = pan(self.viewport, 0.0, -scroll, &self.layout);
            }
        }
        self.space_pan = ui.input(|i| i.key_down(egui::Key::Space));
        let pan_active = self.space_pan
            || ui.input(|i| i.pointer.middle_down())
            || (resp.dragged() && self.panning);
        if resp.drag_started() && !ui.input(|i| i.modifiers.command) {
            match self.interaction {
                ViewerInteractionMode::Normal => {
                    self.panning = self.space_pan || ui.input(|i| i.pointer.middle_down());
                }
                ViewerInteractionMode::Crop => {
                    self.panning = false;
                    if self.crop_drag.is_none() {
                        let pos = resp.interact_pointer_pos().unwrap_or_default();
                        let page = self.page_at_screen(pos, rect);
                        self.crop_drag = Some((
                            ScreenPoint {
                                x: pos.x - rect.min.x,
                                y: pos.y - rect.min.y,
                            },
                            ScreenPoint {
                                x: pos.x - rect.min.x,
                                y: pos.y - rect.min.y,
                            },
                            page,
                        ));
                    }
                }
                ViewerInteractionMode::Signature => {
                    self.panning = false;
                    if self.signature_drag.is_none() {
                        let pos = resp.interact_pointer_pos().unwrap_or_default();
                        let page = self.page_at_screen(pos, rect);
                        self.signature_drag = Some((
                            ScreenPoint {
                                x: pos.x - rect.min.x,
                                y: pos.y - rect.min.y,
                            },
                            ScreenPoint {
                                x: pos.x - rect.min.x,
                                y: pos.y - rect.min.y,
                            },
                            page,
                        ));
                    }
                }
            }
        }
        if resp.dragged() && pan_active {
            let delta = resp.drag_delta();
            self.viewport = pan(self.viewport, delta.x, delta.y, &self.layout);
            self.crop_drag = None;
        } else if self.interaction == ViewerInteractionMode::Crop
            && let Some((start, _, page)) = self.crop_drag
        {
            if let Some(pos) = resp.interact_pointer_pos() {
                let end = ScreenPoint {
                    x: pos.x - rect.min.x,
                    y: pos.y - rect.min.y,
                };
                self.crop_drag = Some((start, end, page));
            }
            if resp.drag_stopped()
                && let Some((start, end, page)) = self.crop_drag
            {
                self.apply_crop(start, end, page, rect);
                self.crop_drag = None;
                self.interaction = ViewerInteractionMode::Normal;
            }
        } else if self.interaction == ViewerInteractionMode::Signature
            && let Some((start, _, page)) = self.signature_drag
        {
            if let Some(pos) = resp.interact_pointer_pos() {
                let end = ScreenPoint {
                    x: pos.x - rect.min.x,
                    y: pos.y - rect.min.y,
                };
                self.signature_drag = Some((start, end, page));
            }
            if resp.drag_stopped()
                && let Some((start, end, page)) = self.signature_drag
            {
                self.apply_signature(start, end, page, rect);
                self.signature_drag = None;
                self.interaction = ViewerInteractionMode::Normal;
            }
        }
        if let Some(viewer) = self.viewer.as_mut() {
            let painter = ui.painter_at(rect);
            paint_main_viewer(ctx, viewer, &self.viewport, &self.layout, rect, &painter);
            for (start, end, page) in [self.crop_drag, self.signature_drag].into_iter().flatten() {
                let idx = page.saturating_sub(1) as usize;
                let (sx, sy, _, _) =
                    docsuite_viewer::page_screen_rect(self.viewport, &self.layout, idx);
                let ax = rect.min.x + sx + start.x - self.viewport.offset_x;
                let ay = rect.min.y + sy + start.y - self.viewport.offset_y;
                let bx = rect.min.x + sx + end.x - self.viewport.offset_x;
                let by = rect.min.y + sy + end.y - self.viewport.offset_y;
                painter.rect_stroke(
                    egui::Rect::from_two_pos(egui::pos2(ax, ay), egui::pos2(bx, by)),
                    0.0,
                    egui::Stroke::new(2.0_f32, ACCENT),
                    egui::StrokeKind::Outside,
                );
            }
        }
        self.current_page = visible_page_range(self.viewport, &self.layout).start;
    }

    fn page_at_screen(&self, pos: egui::Pos2, rect: egui::Rect) -> u32 {
        let x = pos.x - rect.min.x;
        let y = pos.y - rect.min.y;
        let range = visible_page_range(self.viewport, &self.layout);
        for page in range {
            let (sx, sy, sw, sh) =
                docsuite_viewer::page_screen_rect(self.viewport, &self.layout, page);
            if x >= sx && x <= sx + sw && y >= sy && y <= sy + sh {
                return page as u32 + 1;
            }
        }
        self.current_page as u32 + 1
    }

    pub(crate) fn apply_page_order(&mut self, order: &[u32]) {
        let Some(pdf) = self.pdf.as_mut() else {
            return;
        };
        let cancel = CancelToken::new();
        self.selection.set_order_pages(order);
        if docsuite_pdf::reorder(pdf, order, &cancel, &None).is_ok()
            && let Ok(bytes) = pdf.to_bytes()
        {
            self.replace_pdf_bytes(bytes);
        }
    }

    fn apply_signature(&mut self, a: ScreenPoint, b: ScreenPoint, page: u32, rect: egui::Rect) {
        let Some(pdf) = self.pdf.as_mut() else {
            return;
        };
        let idx = page.saturating_sub(1) as usize;
        let (sx, sy, _, _) = docsuite_viewer::page_screen_rect(self.viewport, &self.layout, idx);
        let page_h = self
            .layout
            .page_sizes_pt
            .get(idx)
            .map(|(_, h)| *h)
            .unwrap_or(792.0);
        let pdf_rect = screen_drag_to_pdf_rect(
            a,
            b,
            rect.min.x + sx - self.viewport.offset_x,
            rect.min.y + sy - self.viewport.offset_y,
            self.viewport.scale,
            page_h,
        );
        let cancel = CancelToken::new();
        let stamp = SignatureStamp {
            name: "Signed".to_owned(),
            reason: String::new(),
            date: String::new(),
            image: Some(tools::tiny_stamp_png()),
        };
        if visible_signature(pdf, page, pdf_rect, stamp, &cancel, &None).is_ok()
            && let Ok(bytes) = pdf.to_bytes()
        {
            self.replace_pdf_bytes(bytes);
        }
    }

    fn apply_crop(&mut self, a: ScreenPoint, b: ScreenPoint, page: u32, rect: egui::Rect) {
        let Some(pdf) = self.pdf.as_mut() else {
            return;
        };
        let idx = page.saturating_sub(1) as usize;
        let (sx, sy, _, _) = docsuite_viewer::page_screen_rect(self.viewport, &self.layout, idx);
        let page_h = self.layout.page_sizes_pt[idx].1;
        let pdf_rect = screen_drag_to_pdf_rect(
            a,
            b,
            rect.min.x + sx - self.viewport.offset_x,
            rect.min.y + sy - self.viewport.offset_y,
            self.viewport.scale,
            page_h,
        );
        let cancel = CancelToken::new();
        if docsuite_pdf::crop(pdf, &[page], pdf_rect, false, &cancel, &None).is_ok()
            && let Ok(bytes) = pdf.to_bytes()
        {
            self.replace_pdf_bytes(bytes);
        }
    }

    pub(crate) fn rotate_selected(&mut self, degrees: i32) {
        let pages = self.selection.selected_vec();
        if pages.is_empty() {
            return;
        }
        let Some(pdf) = self.pdf.as_mut() else {
            return;
        };
        let cancel = CancelToken::new();
        if docsuite_pdf::rotate(pdf, &pages, degrees, &cancel, &None).is_ok()
            && let Ok(bytes) = pdf.to_bytes()
        {
            self.replace_pdf_bytes(bytes);
        }
    }

    pub(crate) fn delete_selected(&mut self) {
        let pages = self.selection.selected_vec();
        let Some(pdf) = self.pdf.as_mut() else {
            return;
        };
        let cancel = CancelToken::new();
        if docsuite_pdf::delete_pages(pdf, &pages, &cancel, &None).is_ok()
            && let Ok(bytes) = pdf.to_bytes()
        {
            self.replace_pdf_bytes(bytes);
            self.selection.clear();
        }
    }
}
