#![allow(dead_code)]

use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use docsuite_core::{CancelToken, Error, Result};

use crate::tile::{Tile, TileRectPx};

pub struct RenderQueue {
    tx: SyncSender<QueueMsg>,
    join: Mutex<Option<JoinHandle<()>>>,
}

enum QueueMsg {
    Tile {
        page: usize,
        zoom: f32,
        rect: TileRectPx,
        cancel: CancelToken,
        reply: SyncSender<Result<Tile>>,
    },
    Shutdown,
}

pub trait TileRenderer: Send {
    fn render_tile(
        &mut self,
        page: usize,
        zoom: f32,
        tile_rect_px: TileRectPx,
        cancel: &CancelToken,
    ) -> Result<Tile>;
}

impl RenderQueue {
    pub fn spawn<R, F>(mut renderer: R, mut on_reload: F) -> Self
    where
        R: TileRenderer + 'static,
        F: FnMut() -> Result<R> + Send + 'static,
    {
        let (tx, rx) = mpsc::sync_channel::<QueueMsg>(64);
        let join = thread::spawn(move || worker_loop(&mut renderer, &mut on_reload, rx));
        Self {
            tx,
            join: Mutex::new(Some(join)),
        }
    }

    pub fn render_tile(
        &self,
        page: usize,
        zoom: f32,
        rect: TileRectPx,
        cancel: &CancelToken,
    ) -> Result<Tile> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.tx
            .send(QueueMsg::Tile {
                page,
                zoom,
                rect,
                cancel: cancel.clone(),
                reply: reply_tx,
            })
            .map_err(|_| Error::Backend("render queue closed".to_owned()))?;
        reply_rx
            .recv()
            .map_err(|_| Error::Backend("render reply dropped".to_owned()))?
    }
}

impl Drop for RenderQueue {
    fn drop(&mut self) {
        let _ = self.tx.send(QueueMsg::Shutdown);
        if let Ok(mut guard) = self.join.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

fn worker_loop<R, F>(renderer: &mut R, _on_reload: &mut F, rx: Receiver<QueueMsg>)
where
    R: TileRenderer,
    F: FnMut() -> Result<R>,
{
    while let Ok(msg) = rx.recv() {
        match msg {
            QueueMsg::Shutdown => break,
            QueueMsg::Tile {
                page,
                zoom,
                rect,
                cancel,
                reply,
            } => {
                let out = if cancel.is_cancelled() {
                    Err(Error::Cancelled)
                } else {
                    renderer.render_tile(page, zoom, rect, &cancel)
                };
                let _ = reply.send(out);
            }
        }
    }
}

pub struct CountingRenderer {
    pub calls: Arc<Mutex<usize>>,
    pub sleep_ms: u64,
}

impl TileRenderer for CountingRenderer {
    fn render_tile(
        &mut self,
        _page: usize,
        _zoom: f32,
        tile_rect_px: TileRectPx,
        cancel: &CancelToken,
    ) -> Result<Tile> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if self.sleep_ms > 0 {
            for _ in 0..self.sleep_ms {
                if cancel.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        if let Ok(mut n) = self.calls.lock() {
            *n += 1;
        }
        let w = tile_rect_px.width.max(1.0) as u32;
        let h = tile_rect_px.height.max(1.0) as u32;
        Ok(Tile {
            width: w,
            height: h,
            bytes: vec![0u8; (w * h * 4) as usize],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_skips_cancelled_before_render() {
        let calls = Arc::new(Mutex::new(0usize));
        let queue = RenderQueue::spawn(
            CountingRenderer {
                calls: Arc::clone(&calls),
                sleep_ms: 0,
            },
            || {
                Ok(CountingRenderer {
                    calls: Arc::new(Mutex::new(0)),
                    sleep_ms: 0,
                })
            },
        );
        let cancel = CancelToken::new();
        cancel.cancel();
        let rect = TileRectPx {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 8.0,
        };
        let out = queue.render_tile(0, 1.0, rect, &cancel);
        assert!(matches!(out, Err(Error::Cancelled)));
        assert_eq!(*calls.lock().expect("lock"), 0);
    }

    #[test]
    fn queue_renders_tile() {
        let calls = Arc::new(Mutex::new(0usize));
        let queue = RenderQueue::spawn(
            CountingRenderer {
                calls: Arc::clone(&calls),
                sleep_ms: 2,
            },
            || {
                Ok(CountingRenderer {
                    calls: Arc::new(Mutex::new(0)),
                    sleep_ms: 0,
                })
            },
        );
        let cancel = CancelToken::new();
        let rect = TileRectPx {
            x: 0.0,
            y: 0.0,
            width: 4.0,
            height: 4.0,
        };
        let tile = queue.render_tile(0, 1.0, rect, &cancel).expect("tile");
        assert_eq!(tile.width, 4);
        assert_eq!(*calls.lock().expect("lock"), 1);
    }
}
