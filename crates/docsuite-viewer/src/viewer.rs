use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use docsuite_core::{Error, JobHandle, JobRunner, Priority, Result};

use crate::backend::RenderBackend;
use crate::cache::TileCache;
use crate::tile::{Tile, TileKey, scale_for_zoom_bucket};
use crate::viewport::{
    PageLayout, Viewport, prefetch_tile_keys, tile_rect_px_for_key, visible_tile_keys,
};

pub struct ReadyTile {
    pub key: TileKey,
    pub tile: Tile,
}

pub struct Viewer {
    doc_id: u64,
    backend: Arc<dyn RenderBackend>,
    layout: PageLayout,
    cache: TileCache,
    runner: JobRunner,
    tile_size: u32,
    in_flight: HashMap<TileKey, JobHandle<Tile>>,
}

impl Viewer {
    pub fn new(
        doc_id: u64,
        backend: Arc<dyn RenderBackend>,
        layout: PageLayout,
        cache_budget: usize,
        tile_size: u32,
    ) -> Result<Self> {
        let tile_size = if tile_size == 512 { 512 } else { 256 };
        Ok(Self {
            doc_id,
            backend,
            layout,
            cache: TileCache::new(cache_budget),
            runner: JobRunner::builder().workers(2).build(),
            tile_size,
            in_flight: HashMap::new(),
        })
    }

    pub fn with_runner(
        doc_id: u64,
        backend: Arc<dyn RenderBackend>,
        layout: PageLayout,
        cache_budget: usize,
        tile_size: u32,
        runner: JobRunner,
    ) -> Self {
        let tile_size = if tile_size == 512 { 512 } else { 256 };
        Self {
            doc_id,
            backend,
            layout,
            cache: TileCache::new(cache_budget),
            runner,
            tile_size,
            in_flight: HashMap::new(),
        }
    }

    pub fn cache(&self) -> &TileCache {
        &self.cache
    }

    pub fn update(&mut self, vp: Viewport) {
        let visible: HashSet<TileKey> =
            visible_tile_keys(self.doc_id, vp, &self.layout, self.tile_size)
                .into_iter()
                .collect();
        let prefetch: Vec<TileKey> =
            prefetch_tile_keys(self.doc_id, vp, &self.layout, self.tile_size);
        let want: HashSet<TileKey> = prefetch.into_iter().collect();

        let stale: Vec<TileKey> = self
            .in_flight
            .keys()
            .copied()
            .filter(|k| !want.contains(k))
            .collect();
        for key in stale {
            if let Some(handle) = self.in_flight.remove(&key) {
                handle.cancel();
            }
        }

        for key in want {
            if self.cache.peek(&key).is_some() {
                continue;
            }
            if self.in_flight.contains_key(&key) {
                continue;
            }
            self.submit_tile(key, visible.contains(&key));
        }
    }

    fn submit_tile(&mut self, key: TileKey, high: bool) {
        let rect = tile_rect_px_for_key(&self.layout, key, self.tile_size);
        let zoom = scale_for_zoom_bucket(key.zoom_bucket);
        let page = key.page;
        let backend = Arc::clone(&self.backend);
        let priority = if high { Priority::High } else { Priority::Low };
        let handle = self.runner.submit(priority, move |ctx| {
            backend.render_tile(page, zoom, rect, ctx.cancel_token())
        });
        self.in_flight.insert(key, handle);
    }

    pub fn poll_ready(&mut self) -> Vec<ReadyTile> {
        let mut ready = Vec::new();
        let keys: Vec<TileKey> = self.in_flight.keys().copied().collect();
        for key in keys {
            let Some(handle) = self.in_flight.get(&key) else {
                continue;
            };
            let Some(result) = handle.try_result() else {
                continue;
            };
            let _ = self.in_flight.remove(&key);
            match result {
                Ok(tile) => {
                    self.cache.put(key, tile.clone());
                    ready.push(ReadyTile { key, tile });
                }
                Err(Error::Cancelled) => {}
                Err(_) => {}
            }
        }
        ready
    }

    pub fn cached_tile(&mut self, key: &TileKey) -> Option<Tile> {
        self.cache.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::RenderBackend;
    use crate::tile::{Tile, TileRectPx};
    use docsuite_core::CancelToken;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct SlowBackend {
        calls: AtomicUsize,
        page_sizes: Vec<(f32, f32)>,
    }

    impl SlowBackend {
        fn new(page_sizes: Vec<(f32, f32)>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                page_sizes,
            }
        }
    }

    impl RenderBackend for SlowBackend {
        fn page_count(&self) -> Result<usize> {
            Ok(self.page_sizes.len())
        }

        fn page_size(&self, page: usize) -> Result<(f32, f32)> {
            self.page_sizes
                .get(page)
                .copied()
                .ok_or_else(|| Error::InvalidInput("page".to_owned()))
        }

        fn render_tile(
            &self,
            _page: usize,
            _zoom: f32,
            tile_rect_px: TileRectPx,
            cancel: &CancelToken,
        ) -> Result<Tile> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            for _ in 0..200 {
                cancel.check()?;
                std::thread::sleep(Duration::from_millis(2));
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

    #[test]
    fn scheduler_cancels_stale_jobs() {
        let backend = Arc::new(SlowBackend::new(vec![(612.0, 792.0)]));
        let layout = PageLayout {
            page_sizes_pt: vec![(612.0, 792.0)],
            gap_pt: 0.0,
        };
        let runner = JobRunner::builder().workers(1).build();
        let mut viewer = Viewer::with_runner(1, backend, layout, 64 * 1024 * 1024, 256, runner);
        let vp0 = Viewport {
            width_px: 800.0,
            height_px: 600.0,
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            dpi: 72.0,
        };
        viewer.update(vp0);
        let vp1 = Viewport {
            offset_x: 5000.0,
            offset_y: 5000.0,
            ..vp0
        };
        viewer.update(vp1);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let _ = viewer.poll_ready();
            if viewer.in_flight.is_empty() || std::time::Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let layout = PageLayout {
            page_sizes_pt: vec![(612.0, 792.0)],
            gap_pt: 0.0,
        };
        assert!(viewer.in_flight.len() <= visible_tile_keys(1, vp1, &layout, 256).len() + 2);
    }
}
