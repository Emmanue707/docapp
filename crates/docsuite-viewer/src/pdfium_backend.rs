#[cfg(feature = "pdfium")]
mod imp {
    use std::collections::VecDeque;
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::mpsc::{Receiver, SyncSender};
    use std::thread::{self, JoinHandle};

    use docsuite_core::{CancelToken, Error, Result};
    use pdfium_render::prelude::*;

    use crate::backend::RenderBackend;
    use crate::pdfium_bind::bind_pdfium;
    use crate::tile::{Tile, TileRectPx};

    struct PageCache {
        order: VecDeque<usize>,
        cap: usize,
    }

    impl PageCache {
        fn new(cap: usize) -> Self {
            Self {
                order: VecDeque::new(),
                cap: cap.max(1),
            }
        }

        fn touch(&mut self, page: usize) {
            if let Some(pos) = self.order.iter().position(|&p| p == page) {
                self.order.remove(pos);
            }
            self.order.push_front(page);
            while self.order.len() > self.cap {
                self.order.pop_back();
            }
        }
    }

    struct PdfiumWorker {
        pdfium: Pdfium,
        bytes: Vec<u8>,
        page_count: usize,
        page_sizes: Vec<(f32, f32)>,
        pages: PageCache,
    }

    impl PdfiumWorker {
        fn open(bytes: Vec<u8>) -> Result<Self> {
            let pdfium = bind_pdfium()?;
            let (page_count, page_sizes) = {
                let document = pdfium
                    .load_pdf_from_byte_slice(&bytes, None)
                    .map_err(|e| Error::Backend(e.to_string()))?;
                let page_count = document.pages().len() as usize;
                let mut page_sizes = Vec::with_capacity(page_count);
                for i in 0..page_count {
                    let page = document
                        .pages()
                        .get(i as u16)
                        .map_err(|e| Error::Backend(e.to_string()))?;
                    page_sizes.push((page.width().value, page.height().value));
                }
                (page_count, page_sizes)
            };
            Ok(Self {
                pdfium,
                bytes,
                page_count,
                page_sizes,
                pages: PageCache::new(6),
            })
        }

        fn reload(&mut self, bytes: Vec<u8>) -> Result<()> {
            *self = Self::open(bytes)?;
            Ok(())
        }

        fn with_document<F, T>(&mut self, work: F) -> Result<T>
        where
            F: FnOnce(PdfDocument<'_>) -> Result<T>,
        {
            let document = self
                .pdfium
                .load_pdf_from_byte_slice(&self.bytes, None)
                .map_err(|e| Error::Backend(e.to_string()))?;
            work(document)
        }

        fn render_tile(
            &mut self,
            page: usize,
            zoom: f32,
            tile_rect_px: TileRectPx,
            cancel: &CancelToken,
        ) -> Result<Tile> {
            cancel.check()?;
            if zoom <= 0.0 {
                return Err(Error::InvalidInput("zoom".to_owned()));
            }
            self.pages.touch(page);
            let h_pt = self
                .page_sizes
                .get(page)
                .map(|(_, h)| *h)
                .ok_or_else(|| Error::InvalidInput("page".to_owned()))?;
            let tw = tile_rect_px.width.round().max(1.0) as i32;
            let th = tile_rect_px.height.round().max(1.0) as i32;
            let x0_pt = tile_rect_px.x / zoom;
            let y_top_pt = tile_rect_px.y / zoom;
            let y0_pt = h_pt - y_top_pt - tile_rect_px.height / zoom;
            cancel.check()?;
            self.with_document(|document| {
                let page = document
                    .pages()
                    .get(page as u16)
                    .map_err(|e| Error::Backend(e.to_string()))?;
                let config = PdfRenderConfig::new()
                    .set_fixed_width(tw)
                    .set_fixed_height(th)
                    .scale(zoom, zoom)
                    .map_err(|e| Error::Backend(e.to_string()))?
                    .translate(PdfPoints::new(-x0_pt), PdfPoints::new(-y0_pt))
                    .map_err(|e| Error::Backend(e.to_string()))?;
                cancel.check()?;
                let bitmap = page
                    .render_with_config(&config)
                    .map_err(|e| Error::Backend(e.to_string()))?;
                let image = bitmap.as_image().into_rgba8();
                let (bw, bh) = image.dimensions();
                Ok(Tile {
                    width: bw,
                    height: bh,
                    bytes: image.into_raw(),
                })
            })
        }
    }

    #[expect(dead_code)]
    enum WorkerMsg {
        Reload {
            bytes: Vec<u8>,
            reply: SyncSender<Result<()>>,
        },
        PageCount {
            reply: SyncSender<Result<usize>>,
        },
        PageSize {
            page: usize,
            reply: SyncSender<Result<(f32, f32)>>,
        },
        Tile {
            page: usize,
            zoom: f32,
            rect: TileRectPx,
            cancel: CancelToken,
            reply: SyncSender<Result<Tile>>,
        },
        Shutdown,
    }

    pub struct PdfiumBackend {
        tx: SyncSender<WorkerMsg>,
        join: Mutex<Option<JoinHandle<()>>>,
    }

    impl PdfiumBackend {
        pub fn open_path(path: &Path) -> Result<Self> {
            let bytes = std::fs::read(path).map_err(Error::Io)?;
            Self::from_bytes(bytes)
        }

        pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
            bind_pdfium()?;
            let (tx, rx) = std::sync::mpsc::sync_channel::<WorkerMsg>(64);
            let init = bytes;
            let join = thread::spawn(move || {
                let mut worker = match PdfiumWorker::open(init) {
                    Ok(w) => w,
                    Err(_) => return,
                };
                worker_loop(&mut worker, rx);
            });
            Ok(Self {
                tx,
                join: Mutex::new(Some(join)),
            })
        }

        fn ask_count(&self) -> Result<usize> {
            let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
            self.tx
                .send(WorkerMsg::PageCount { reply: reply_tx })
                .map_err(|_| Error::Backend("pdfium worker closed".to_owned()))?;
            reply_rx
                .recv()
                .map_err(|_| Error::Backend("pdfium count dropped".to_owned()))?
        }

        fn ask_page_size(&self, page: usize) -> Result<(f32, f32)> {
            let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
            self.tx
                .send(WorkerMsg::PageSize {
                    page,
                    reply: reply_tx,
                })
                .map_err(|_| Error::Backend("pdfium worker closed".to_owned()))?;
            reply_rx
                .recv()
                .map_err(|_| Error::Backend("pdfium size dropped".to_owned()))?
        }

        fn ask_tile(
            &self,
            page: usize,
            zoom: f32,
            rect: TileRectPx,
            cancel: &CancelToken,
        ) -> Result<Tile> {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
            self.tx
                .send(WorkerMsg::Tile {
                    page,
                    zoom,
                    rect,
                    cancel: cancel.clone(),
                    reply: reply_tx,
                })
                .map_err(|_| Error::Backend("pdfium worker closed".to_owned()))?;
            reply_rx
                .recv()
                .map_err(|_| Error::Backend("pdfium tile dropped".to_owned()))?
        }
    }

    impl Drop for PdfiumBackend {
        fn drop(&mut self) {
            let _ = self.tx.send(WorkerMsg::Shutdown);
            if let Ok(mut guard) = self.join.lock()
                && let Some(handle) = guard.take()
            {
                let _ = handle.join();
            }
        }
    }

    fn worker_loop(worker: &mut PdfiumWorker, rx: Receiver<WorkerMsg>) {
        while let Ok(msg) = rx.recv() {
            match msg {
                WorkerMsg::Shutdown => break,
                WorkerMsg::Reload { bytes, reply } => {
                    let out = worker.reload(bytes);
                    let _ = reply.send(out);
                }
                WorkerMsg::PageCount { reply } => {
                    let _ = reply.send(Ok(worker.page_count));
                }
                WorkerMsg::PageSize { page, reply } => {
                    let out = worker
                        .page_sizes
                        .get(page)
                        .copied()
                        .ok_or_else(|| Error::InvalidInput("page".to_owned()));
                    let _ = reply.send(out);
                }
                WorkerMsg::Tile {
                    page,
                    zoom,
                    rect,
                    cancel,
                    reply,
                } => {
                    let out = if cancel.is_cancelled() {
                        Err(Error::Cancelled)
                    } else {
                        worker.render_tile(page, zoom, rect, &cancel)
                    };
                    let _ = reply.send(out);
                }
            }
        }
    }

    impl RenderBackend for PdfiumBackend {
        fn page_count(&self) -> Result<usize> {
            self.ask_count()
        }

        fn page_size(&self, page: usize) -> Result<(f32, f32)> {
            self.ask_page_size(page)
        }

        fn render_tile(
            &self,
            page: usize,
            zoom: f32,
            tile_rect_px: TileRectPx,
            cancel: &CancelToken,
        ) -> Result<Tile> {
            self.ask_tile(page, zoom, tile_rect_px, cancel)
        }
    }
}

#[cfg(feature = "pdfium")]
pub use imp::PdfiumBackend;
