#[cfg(feature = "pdfium")]
mod imp {
    use std::path::Path;

    use docsuite_core::{CancelToken, Error, Result};
    use pdfium_render::prelude::*;

    use crate::backend::RenderBackend;
    use crate::tile::{Tile, TileRectPx};

    pub struct PdfiumBackend {
        bytes: Vec<u8>,
    }

    impl PdfiumBackend {
        pub fn open_path(path: &Path) -> Result<Self> {
            let bytes = std::fs::read(path).map_err(Error::Io)?;
            let pdfium = open_pdfium()?;
            pdfium
                .load_pdf_from_byte_slice(&bytes, None)
                .map_err(|e| Error::Backend(e.to_string()))?;
            Ok(Self { bytes })
        }
    }

    fn open_pdfium() -> Result<Pdfium> {
        Ok(Pdfium::new(
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
                .or_else(|_| Pdfium::bind_to_system_library())
                .map_err(|e| Error::Backend(e.to_string()))?,
        ))
    }

    fn with_document<F, T>(bytes: &[u8], work: F) -> Result<T>
    where
        F: FnOnce(PdfDocument<'_>) -> Result<T>,
    {
        let pdfium = open_pdfium()?;
        let document = pdfium
            .load_pdf_from_byte_slice(bytes, None)
            .map_err(|e| Error::Backend(e.to_string()))?;
        work(document)
    }

    impl RenderBackend for PdfiumBackend {
        fn page_count(&self) -> Result<usize> {
            with_document(&self.bytes, |document| Ok(document.pages().len() as usize))
        }

        fn page_size(&self, page: usize) -> Result<(f32, f32)> {
            with_document(&self.bytes, |document| {
                let page = document
                    .pages()
                    .get(page as u16)
                    .map_err(|e| Error::Backend(e.to_string()))?;
                Ok((page.width().value, page.height().value))
            })
        }

        fn render_tile(
            &self,
            page: usize,
            zoom: f32,
            tile_rect_px: TileRectPx,
            cancel: &CancelToken,
        ) -> Result<Tile> {
            cancel.check()?;
            with_document(&self.bytes, |document| {
                let page = document
                    .pages()
                    .get(page as u16)
                    .map_err(|e| Error::Backend(e.to_string()))?;
                let config = PdfRenderConfig::new()
                    .set_target_width((tile_rect_px.width * zoom).round().max(1.0) as i32)
                    .set_maximum_height((tile_rect_px.height * zoom).round().max(1.0) as i32);
                cancel.check()?;
                let bitmap = page
                    .render_with_config(&config)
                    .map_err(|e| Error::Backend(e.to_string()))?;
                let image = bitmap.as_image().into_rgba8();
                let (w, h) = image.dimensions();
                Ok(Tile {
                    width: w,
                    height: h,
                    bytes: image.into_raw(),
                })
            })
        }
    }
}

#[cfg(feature = "pdfium")]
pub use imp::PdfiumBackend;
