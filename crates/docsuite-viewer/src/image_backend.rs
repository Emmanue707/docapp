use std::sync::Mutex;

use docsuite_core::{CancelToken, Error, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbaImage};

use crate::backend::RenderBackend;
use crate::tile::{Tile, TileRectPx};

pub struct ImageBackend {
    image: Mutex<DynamicImage>,
    width_pt: f32,
    height_pt: f32,
}

impl ImageBackend {
    pub fn from_bytes(data: &[u8], dpi: f32) -> Result<Self> {
        let img = image::load_from_memory(data).map_err(|e| Error::Backend(e.to_string()))?;
        let (w, h) = img.dimensions();
        let dpi = dpi.max(1.0);
        let width_pt = w as f32 * 72.0 / dpi;
        let height_pt = h as f32 * 72.0 / dpi;
        Ok(Self {
            image: Mutex::new(img),
            width_pt,
            height_pt,
        })
    }

    pub fn from_rgba(img: RgbaImage, dpi: f32) -> Self {
        let (w, h) = img.dimensions();
        let dpi = dpi.max(1.0);
        let width_pt = w as f32 * 72.0 / dpi;
        let height_pt = h as f32 * 72.0 / dpi;
        Self {
            image: Mutex::new(DynamicImage::ImageRgba8(img)),
            width_pt,
            height_pt,
        }
    }
}

impl RenderBackend for ImageBackend {
    fn page_count(&self) -> Result<usize> {
        Ok(1)
    }

    fn page_size(&self, page: usize) -> Result<(f32, f32)> {
        if page != 0 {
            return Err(Error::InvalidInput("page out of range".to_owned()));
        }
        Ok((self.width_pt, self.height_pt))
    }

    fn render_tile(
        &self,
        page: usize,
        zoom: f32,
        tile_rect_px: TileRectPx,
        cancel: &CancelToken,
    ) -> Result<Tile> {
        cancel.check()?;
        if page != 0 {
            return Err(Error::InvalidInput("page out of range".to_owned()));
        }
        let guard = self
            .image
            .lock()
            .map_err(|_| Error::Backend("image lock poisoned".to_owned()))?;
        let (src_w, src_h) = guard.dimensions();
        if src_w == 0 || src_h == 0 {
            return Err(Error::InvalidInput("empty image".to_owned()));
        }
        let page_w_px = self.width_pt * zoom;
        let page_h_px = self.height_pt * zoom;
        let sx0 = (tile_rect_px.x * src_w as f32 / page_w_px).floor().max(0.0) as u32;
        let sy0 = (tile_rect_px.y * src_h as f32 / page_h_px).floor().max(0.0) as u32;
        let sx1 = ((tile_rect_px.x + tile_rect_px.width) * src_w as f32 / page_w_px).ceil() as u32;
        let sy1 = ((tile_rect_px.y + tile_rect_px.height) * src_h as f32 / page_h_px).ceil() as u32;
        let sx1 = sx1.min(src_w).max(sx0 + 1);
        let sy1 = sy1.min(src_h).max(sy0 + 1);
        cancel.check()?;
        let crop = guard.crop_imm(sx0, sy0, sx1 - sx0, sy1 - sy0).to_rgba8();
        let tw = tile_rect_px.width.round().max(1.0) as u32;
        let th = tile_rect_px.height.round().max(1.0) as u32;
        let scaled = image::imageops::resize(&crop, tw, th, FilterType::Triangle);
        cancel.check()?;
        Ok(Tile {
            width: tw,
            height: th,
            bytes: scaled.into_raw(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::TileRectPx;

    #[test]
    fn tile_pixels_match_cropped_region() {
        let mut img = RgbaImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                img.put_pixel(x, y, image::Rgba([x as u8, y as u8, 10, 255]));
            }
        }
        let backend = ImageBackend::from_rgba(img, 72.0);
        let cancel = CancelToken::new();
        let tile = backend
            .render_tile(
                0,
                1.0,
                TileRectPx {
                    x: 0.0,
                    y: 0.0,
                    width: 32.0,
                    height: 32.0,
                },
                &cancel,
            )
            .expect("tile");
        assert_eq!(tile.width, 32);
        assert_eq!(tile.height, 32);
        assert_eq!(tile.bytes[0], 0);
        assert_eq!(tile.bytes[1], 0);
        let idx = (16 * 32 + 16) as usize * 4;
        assert_eq!(tile.bytes[idx], 16);
        assert_eq!(tile.bytes[idx + 1], 16);
    }
}
