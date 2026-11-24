use docsuite_core::{CancelToken, Result};

use crate::tile::{Tile, TileRectPx};

pub trait RenderBackend: Send + Sync {
    fn page_count(&self) -> Result<usize>;
    fn page_size(&self, page: usize) -> Result<(f32, f32)>;
    fn render_tile(
        &self,
        page: usize,
        zoom: f32,
        tile_rect_px: TileRectPx,
        cancel: &CancelToken,
    ) -> Result<Tile>;
}
