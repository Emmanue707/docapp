use std::path::PathBuf;
use std::sync::Arc;

use docsuite_convert::extract::extract_page;
use docsuite_core::{CancelToken, Error, JobHandle, JobRunner, Progress, ProgressSink};
use docsuite_pdf::{Pdf, PdfRect};
use docsuite_viewer::{
    DEFAULT_CACHE_DESKTOP, ImageBackend, PageLayout, PdfiumBackend, RenderBackend, Viewer,
    Viewport, fit_width, pan, visible_page_range, zoom_around_point,
};
use eframe::egui;

use crate::recent::{load_recent, push_recent};
use crate::state::{
    PageSelection, ScreenPoint, UndoStack, parse_tool_split_ranges, screen_drag_to_pdf_rect,
};

const UNDO_CAP: usize = 20;

pub struct DocApp {
    runner: JobRunner,
    progress: Arc<std::sync::Mutex<Option<Progress>>>,
    cancel: CancelToken,
    job: Option<JobHandle<()>>,
    pdf_bytes: Option<Vec<u8>>,
    pdf: Option<Pdf>,
    path: Option<PathBuf>,
    dirty: bool,
    undo: UndoStack,
    selection: PageSelection,
    viewer: Option<Viewer>,
    layout: PageLayout,
    viewport: Viewport,
    current_page: usize,
    search: String,
    search_hits: Vec<(u32, PdfRect)>,
    merge_files: Vec<PathBuf>,
    split_ranges: String,
    split_every: String,
    tool_message: String,
    models_dir: PathBuf,
    models_missing: bool,
    recent: Vec<PathBuf>,
    panning: bool,
    space_pan: bool,
    crop_drag: Option<(ScreenPoint, ScreenPoint, u32)>,
    signature_pending: Option<Vec<u8>>,
}

impl Default for DocApp {
    fn default() -> Self {
        let models_dir = docsuite_models::default_dir();
        let models_missing = !docsuite_models::status(&models_dir).ready();
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
            search: String::new(),
            search_hits: Vec::new(),
            merge_files: Vec::new(),
            split_ranges: String::new(),
            split_every: String::new(),
            tool_message: String::new(),
            models_dir,
            models_missing,
            recent: load_recent(),
            panning: false,
            space_pan: false,
            crop_drag: None,
            signature_pending: None,
        }
    }
}

impl DocApp {
    fn sync_models(&mut self) {
        self.models_missing = !docsuite_models::status(&self.models_dir).ready();
    }

    fn open_path(&mut self, path: PathBuf) {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.tool_message = format!("open failed: {e}");
                return;
            }
        };
        if bytes.starts_with(b"%PDF") {
            match Pdf::load_bytes(&bytes) {
                Ok(pdf) => self.load_pdf(path, bytes, pdf),
                Err(e) => self.tool_message = format!("{e}"),
            }
        } else if image::guess_format(&bytes).is_ok() {
            self.load_image(path, bytes);
        } else {
            self.tool_message = "unsupported file type".to_owned();
        }
    }

    fn load_pdf(&mut self, path: PathBuf, bytes: Vec<u8>, pdf: Pdf) {
        let backend: Arc<dyn docsuite_viewer::RenderBackend> =
            match PdfiumBackend::from_bytes(bytes.clone()) {
                Ok(b) => Arc::new(b),
                Err(e) => {
                    self.tool_message = format!("{e}");
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
        push_recent(&path);
        self.recent = load_recent();
        self.tool_message.clear();
    }

    fn load_image(&mut self, path: PathBuf, bytes: Vec<u8>) {
        let backend = match ImageBackend::from_bytes(&bytes, 96.0) {
            Ok(b) => Arc::new(b),
            Err(e) => {
                self.tool_message = format!("{e}");
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
        push_recent(&path);
        self.recent = load_recent();
        self.tool_message = "opened image (export as PDF via Tools)".to_owned();
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
            Err(e) => self.tool_message = format!("{e}"),
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        let Some(pdf) = self.pdf.as_mut() else {
            self.tool_message = "nothing to save".to_owned();
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
            }
            Err(e) => self.tool_message = format!("save failed: {e}"),
        }
    }

    fn run_bg<F>(&mut self, stage: &'static str, work: F)
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
                Ok(()) => self.tool_message = "done".to_owned(),
                Err(Error::Cancelled) => self.tool_message = "cancelled".to_owned(),
                Err(e) => self.tool_message = format!("{e}"),
            }
        }
    }

    fn run_search(&mut self) {
        self.search_hits.clear();
        let q = self.search.trim().to_ascii_lowercase();
        if q.is_empty() {
            return;
        }
        let Some(pdf) = self.pdf.as_ref() else {
            return;
        };
        let doc = pdf.lopdf_document();
        for pn in 1..=pdf.page_count() {
            let Ok(page_id) = pdf.page_id(pn) else {
                continue;
            };
            let Ok(page) = extract_page(doc, page_id) else {
                continue;
            };
            for line in &page.text {
                for run in &line.runs {
                    if run.text.to_ascii_lowercase().contains(&q) {
                        self.search_hits.push((
                            pn,
                            PdfRect {
                                llx: run.x,
                                lly: run.y,
                                urx: run.x + run.width,
                                ury: run.y + run.font_size,
                            },
                        ));
                    }
                }
            }
        }
        self.tool_message = format!("{} hits", self.search_hits.len());
    }
}

impl eframe::App for DocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job();
        self.sync_models();
        ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(10.0, 8.0);
        });

        if ctx.input(|i| i.viewport().close_requested()) && self.dirty {
            self.tool_message = "unsaved changes".to_owned();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("documents", &["pdf", "png", "jpg", "jpeg", "webp"])
                        .pick_file()
                {
                    self.open_path(path);
                }
                if ui
                    .add_enabled(self.pdf.is_some(), egui::Button::new("Save"))
                    .clicked()
                    && let Some(path) = self.path.clone()
                {
                    self.save_to(path);
                }
                if ui
                    .add_enabled(self.pdf.is_some(), egui::Button::new("Save as"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name("document.pdf")
                        .save_file()
                {
                    self.save_to(path);
                }
                ui.separator();
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
                    .add_enabled(self.undo.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                    && let Some(cur) = self.pdf_bytes.as_ref()
                    && let Some(bytes) = self.undo.redo(cur)
                    && let Ok(pdf) = Pdf::load_bytes(&bytes)
                    && let Some(path) = self.path.clone()
                {
                    self.load_pdf(path, bytes, pdf);
                }
                ui.separator();
                if ui.button("−").clicked() {
                    let focal =
                        egui::pos2(self.viewport.width_px * 0.5, self.viewport.height_px * 0.5);
                    self.viewport = zoom_around_point(
                        self.viewport,
                        self.viewport.scale / 1.2,
                        focal.x,
                        focal.y,
                    );
                }
                if ui.button("+").clicked() {
                    let focal =
                        egui::pos2(self.viewport.width_px * 0.5, self.viewport.height_px * 0.5);
                    self.viewport = zoom_around_point(
                        self.viewport,
                        self.viewport.scale * 1.2,
                        focal.x,
                        focal.y,
                    );
                }
                if ui.button("Fit width").clicked() {
                    self.viewport = fit_width(self.viewport, &self.layout);
                }
                if !self.layout.page_sizes_pt.is_empty() {
                    ui.label(format!(
                        "page {}/{}",
                        self.current_page + 1,
                        self.layout.page_sizes_pt.len()
                    ));
                }
                ui.label("Search:");
                let submit = ui.text_edit_singleline(&mut self.search).lost_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                if submit {
                    self.run_search();
                }
            });
        });

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
                ui.spinner();
            });
        }

        egui::SidePanel::left("thumbs")
            .resizable(true)
            .default_width(120.0)
            .show(ctx, |ui| {
                ui.heading("Pages");
                let thumb_vp = Viewport {
                    width_px: 96.0,
                    height_px: 128.0,
                    scale: 0.15,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    dpi: 96.0,
                };
                if let Some(viewer) = self.viewer.as_mut() {
                    viewer.update(thumb_vp);
                    let _ready = viewer.poll_ready();
                }
                let mut thumb_order: Vec<u32> =
                    (1..=self.layout.page_sizes_pt.len() as u32).collect();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    #[allow(clippy::needless_range_loop)]
                    for idx in 0..thumb_order.len() {
                        let page = thumb_order[idx];
                        let selected = self.selection.selected_vec().contains(&page);
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(selected, format!("Page {page}"))
                                .clicked()
                            {
                                let extend = ui.input(|i| i.modifiers.shift);
                                self.selection.toggle(
                                    page,
                                    extend,
                                    self.layout.page_sizes_pt.len() as u32,
                                );
                            }
                            if ui.small_button("↑").clicked() && idx > 0 {
                                self.selection
                                    .apply_reorder_drag(idx, idx - 1, &mut thumb_order);
                            }
                            if ui.small_button("↓").clicked() && idx + 1 < thumb_order.len() {
                                self.selection
                                    .apply_reorder_drag(idx, idx + 1, &mut thumb_order);
                            }
                        });
                    }
                });
                if ui.button("Apply page order").clicked() {
                    self.apply_page_order(&thumb_order);
                }
                ui.separator();
                if ui.button("Rotate left 90°").clicked() {
                    self.rotate_selected(-90);
                }
                if ui.button("Rotate right 90°").clicked() {
                    self.rotate_selected(90);
                }
                if ui.button("Delete selected").clicked() {
                    self.delete_selected();
                }
                let _ = &mut thumb_order;
            });

        egui::SidePanel::right("tools")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Tools");
                ui.label(&self.tool_message);
                ui.separator();
                ui.collapsing("Merge", |ui| {
                    if ui.button("Add file…").clicked()
                        && let Some(f) = rfd::FileDialog::new().pick_file()
                    {
                        self.merge_files.push(f);
                    }
                    for f in &self.merge_files {
                        ui.label(f.file_name().and_then(|n| n.to_str()).unwrap_or("file"));
                    }
                    if ui.button("Merge to new doc").clicked() {
                        self.merge_pdfs();
                    }
                });
                ui.collapsing("Split", |ui| {
                    ui.text_edit_singleline(&mut self.split_ranges);
                    ui.label("ranges (; groups with ,)");
                    ui.text_edit_singleline(&mut self.split_every);
                    ui.label("every N (optional)");
                    if ui.button("Split").clicked() {
                        self.split_pdf();
                    }
                });
                ui.collapsing("Rotate / Crop / Sign", |ui| {
                    if ui.button("Crop mode (drag on page)").clicked() {
                        self.tool_message = "drag a box on the page".to_owned();
                    }
                    if ui.button("Pick signature image").clicked()
                        && let Some(f) = rfd::FileDialog::new().pick_file()
                        && let Ok(b) = std::fs::read(f)
                    {
                        self.signature_pending = Some(b);
                    }
                    ui.label(if self.signature_pending.is_some() {
                        "signature image ready — drag on page"
                    } else {
                        "no signature image"
                    });
                });
                ui.collapsing("Convert", |ui| {
                    if ui.button("PDF → Word (exact)").clicked() {
                        self.convert_word(docsuite_convert::PdfToWordMode::ExactLook);
                    }
                    if ui.button("PDF → Word (editable)").clicked() {
                        self.convert_word(docsuite_convert::PdfToWordMode::Editable);
                    }
                    if ui.button("PDF → images folder").clicked() {
                        self.export_images();
                    }
                    if ui.button("Image → PDF").clicked() {
                        self.image_to_pdf();
                    }
                });
                ui.collapsing("OCR", |ui| {
                    if self.models_missing {
                        if ui.button("Download OCR models (12 MB)").clicked() {
                            let dir = self.models_dir.clone();
                            self.run_bg("models", move |_cancel, _progress| {
                                docsuite_models::fetch_models(&dir).map_err(|e| e.to_string())
                            });
                        }
                    } else if ui.button("Make searchable PDF").clicked() {
                        self.ocr_searchable();
                    }
                });
                ui.separator();
                ui.label("Recent");
                for r in self.recent.clone() {
                    let label = r.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                    if ui.link(label).clicked() {
                        self.open_path(r);
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
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
                self.panning = self.space_pan || ui.input(|i| i.pointer.middle_down());
                if let Some((_, _, page)) = &self.crop_drag {
                    let _ = page;
                } else if self.crop_drag.is_none() {
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
            if resp.dragged() && pan_active {
                let delta = resp.drag_delta();
                self.viewport = pan(self.viewport, delta.x, delta.y, &self.layout);
                self.crop_drag = None;
            } else if let Some((start, _, page)) = self.crop_drag {
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
                }
            }
            if let Some(viewer) = self.viewer.as_mut() {
                viewer.update(self.viewport);
                let ready = viewer.poll_ready();
                let painter = ui.painter_at(rect);
                for rt in ready {
                    if let Some(tex) = tile_texture(ctx, &rt.tile).map(|h| h.id()) {
                        let (sx, sy, _, _) = docsuite_viewer::page_screen_rect(
                            self.viewport,
                            &self.layout,
                            rt.key.page,
                        );
                        let ts = 256.0;
                        let x = rect.min.x + sx + rt.key.tx as f32 * ts - self.viewport.offset_x;
                        let y = rect.min.y + sy + rt.key.ty as f32 * ts - self.viewport.offset_y;
                        painter.image(
                            tex,
                            egui::Rect::from_min_size(
                                egui::pos2(x, y),
                                egui::vec2(rt.tile.width as f32, rt.tile.height as f32),
                            ),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }
                for (pn, hit) in &self.search_hits {
                    let page = pn.saturating_sub(1) as usize;
                    let (sx, sy, sw, sh) =
                        docsuite_viewer::page_screen_rect(self.viewport, &self.layout, page);
                    let scale = self.viewport.scale;
                    let hx = rect.min.x + sx + hit.llx as f32 * scale - self.viewport.offset_x;
                    let hy = rect.min.y
                        + sy
                        + (self.layout.page_sizes_pt[page].1 - hit.ury as f32) * scale
                        - self.viewport.offset_y;
                    let hw = (hit.urx - hit.llx) as f32 * scale;
                    let hh = (hit.ury - hit.lly) as f32 * scale;
                    painter.rect_stroke(
                        egui::Rect::from_min_size(egui::pos2(hx, hy), egui::vec2(hw, hh)),
                        0.0,
                        egui::Stroke::new(1.0_f32, egui::Color32::YELLOW),
                        egui::StrokeKind::Outside,
                    );
                    let _ = (sw, sh);
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a PDF or image — or drop a file here");
                });
            }
            self.current_page = visible_page_range(self.viewport, &self.layout).start;
        });

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
                    self.open_path(path);
                }
            }
        }

        ctx.request_repaint();
    }
}

impl DocApp {
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

    fn apply_page_order(&mut self, order: &[u32]) {
        let Some(pdf) = self.pdf.as_mut() else {
            return;
        };
        self.selection.set_order_pages(order);
        let cancel = CancelToken::new();
        if docsuite_pdf::reorder(pdf, order, &cancel, &None).is_ok()
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

    fn rotate_selected(&mut self, degrees: i32) {
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

    fn delete_selected(&mut self) {
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

    fn merge_pdfs(&mut self) {
        let files = self.merge_files.clone();
        if files.len() < 2 {
            self.tool_message = "add at least two PDFs".to_owned();
            return;
        }
        self.run_bg("merge", move |cancel, progress| {
            let pdfs: Result<Vec<Pdf>, String> = files
                .iter()
                .map(|p| Pdf::load(p).map_err(|e| e.to_string()))
                .collect();
            let pdfs = pdfs?;
            let mut merged =
                docsuite_pdf::merge(pdfs, &cancel, &progress).map_err(|e| e.to_string())?;
            let bytes = merged.to_bytes().map_err(|e| e.to_string())?;
            let _ = bytes;
            Ok(())
        });
    }

    fn split_pdf(&mut self) {
        let ranges = parse_tool_split_ranges(&self.split_ranges).unwrap_or_default();
        let every = self.split_every.trim().parse::<u32>().ok();
        let path = self.path.clone();
        let Some(path) = path else {
            return;
        };
        self.run_bg("split", move |cancel, progress| {
            let pdf = Pdf::load(&path).map_err(|e| e.to_string())?;
            if let Some(n) = every {
                let _parts = docsuite_pdf::split_every(&pdf, n, &cancel, &progress)
                    .map_err(|e| e.to_string())?;
            } else if !ranges.is_empty() {
                let _parts = docsuite_pdf::split(&pdf, &ranges, &cancel, &progress)
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        });
    }

    fn convert_word(&mut self, mode: docsuite_convert::PdfToWordMode) {
        let path = self.path.clone();
        let Some(input) = path else {
            return;
        };
        let out = input.with_extension("docx");
        self.run_bg("convert", move |cancel, progress| {
            let opts = docsuite_convert::ConvertOptions {
                image: Default::default(),
                pdf_to_word: docsuite_convert::PdfToWordOptions {
                    mode,
                    ..Default::default()
                },
            };
            docsuite_convert::convert(&input, &out, &opts, &cancel, &progress)
                .map_err(|e| e.to_string())
        });
    }

    fn export_images(&mut self) {
        let path = self.path.clone();
        let Some(input) = path else {
            return;
        };
        let out = input.with_extension("pages");
        self.run_bg("render", move |cancel, progress| {
            let opts = docsuite_convert::PdfToImageOptions {
                dpi: 150.0,
                pages: None,
                format: docsuite_convert::Format::Png,
            };
            docsuite_convert::pdf_to_image(&input, &out, &opts, &cancel, &progress)
                .map_err(|e| e.to_string())
        });
    }

    fn image_to_pdf(&mut self) {
        let path = self.path.clone();
        let Some(input) = path else {
            return;
        };
        let out = input.with_extension("pdf");
        self.run_bg("image-pdf", move |cancel, progress| {
            docsuite_convert::image_to_pdf(&input, &out, &Default::default(), &cancel, &progress)
                .map_err(|e| e.to_string())
        });
    }

    fn ocr_searchable(&mut self) {
        let path = self.path.clone();
        let Some(input) = path else {
            return;
        };
        let out = input.with_extension("searchable.pdf");
        let dir = self.models_dir.clone();
        self.run_bg("ocr", move |cancel, progress| {
            let rec = docsuite_ocr::OcrsRecognizer::with_dir(&dir).map_err(|e| e.to_string())?;
            let ocr = std::sync::Arc::new(docsuite_convert::RecognizerPageOcr {
                recognizer: std::sync::Arc::new(rec),
                highlight_low_confidence: false,
            });
            let opts = docsuite_convert::PdfTextOptions {
                pages: None,
                ocr: true,
                page_ocr: Some(ocr),
            };
            docsuite_convert::pdf_to_searchable_pdf(&input, &out, &opts, &cancel, &progress)
                .map_err(|e| e.to_string())
        });
    }
}

fn tile_texture(ctx: &egui::Context, tile: &docsuite_viewer::Tile) -> Option<egui::TextureHandle> {
    let img = egui::ColorImage::from_rgba_unmultiplied(
        [tile.width as usize, tile.height as usize],
        &tile.bytes,
    );
    Some(ctx.load_texture(
        format!("tile_{}_{}", tile.width, tile.height),
        img,
        egui::TextureOptions::LINEAR,
    ))
}
