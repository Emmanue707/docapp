mod backend;
mod cache;
mod image_backend;
#[cfg(feature = "pdfium")]
mod pdfium_backend;
mod tile;
mod viewer;
mod viewport;

pub use backend::RenderBackend;
pub use cache::TileCache;
pub use image_backend::ImageBackend;
#[cfg(feature = "pdfium")]
pub use pdfium_backend::PdfiumBackend;
pub use tile::{SQRT2, Tile, TileKey, TileRectPx, scale_for_zoom_bucket, zoom_bucket_for_scale};
pub use viewer::{ReadyTile, Viewer};
pub use viewport::{
    PageLayout, Viewport, fit_page, fit_width, pan, prefetch_tile_keys, tile_rect_px_for_key,
    visible_page_range, visible_tile_keys, zoom_around_point,
};

pub const DEFAULT_CACHE_DESKTOP: usize = 64 * 1024 * 1024;
pub const DEFAULT_CACHE_MOBILE: usize = 24 * 1024 * 1024;
