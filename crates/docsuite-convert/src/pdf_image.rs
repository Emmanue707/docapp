#[cfg(feature = "pdfium")]
mod imp {
    use std::fs;
    use std::path::{Path, PathBuf};

    use docsuite_core::{CancelToken, Error, ProgressSink, Result};
    use docsuite_pdf::PageRanges;
    use image::ImageFormat;
    use pdfium_render::prelude::*;

    use crate::Format;

    #[derive(Clone, Debug)]
    pub struct PdfToImageOptions {
        pub dpi: f32,
        pub pages: Option<PageRanges>,
        pub format: Format,
    }

    pub fn pdf_to_image(
        input: &Path,
        output: &Path,
        opts: &PdfToImageOptions,
        cancel: &CancelToken,
        progress: &Option<ProgressSink>,
    ) -> Result<()> {
        let bytes = fs::read(input).map_err(Error::Io)?;
        let pdfium = bind_pdfium()?;
        let document = pdfium
            .load_pdf_from_byte_slice(&bytes, None)
            .map_err(|e| Error::Backend(e.to_string()))?;
        let page_count = document.pages().len() as u32;
        let pages = match &opts.pages {
            Some(r) => r.pages(page_count)?,
            None => (1..=page_count).collect(),
        };
        if pages.is_empty() {
            return Err(Error::InvalidInput("no pages selected".to_owned()));
        }
        let multi = pages.len() > 1 || output.extension().is_none();
        if multi {
            fs::create_dir_all(output).map_err(Error::Io)?;
        } else if let Some(parent) = output.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("page");
        let fmt = image_format(opts.format)?;
        let scale = opts.dpi.max(1.0) / 72.0;
        let total = pages.len() as u64;
        for (i, pn) in pages.iter().enumerate() {
            cancel.check()?;
            report(progress, i as u64, total, "render-page");
            let page = document
                .pages()
                .get(pn.saturating_sub(1) as u16)
                .map_err(|e| Error::Backend(e.to_string()))?;
            let w = (page.width().value * scale).round().max(1.0) as i32;
            let h = (page.height().value * scale).round().max(1.0) as i32;
            let config = PdfRenderConfig::new()
                .set_fixed_width(w)
                .set_fixed_height(h)
                .scale(scale, scale)
                .map_err(|e| Error::Backend(e.to_string()))?;
            cancel.check()?;
            let bitmap = page
                .render_with_config(&config)
                .map_err(|e| Error::Backend(e.to_string()))?;
            let img = bitmap.as_image();
            let out_path = if multi {
                output.join(format!(
                    "{stem}-p{pn:04}.{ext}",
                    ext = format_ext(opts.format)
                ))
            } else {
                output.to_path_buf()
            };
            img.save_with_format(&out_path, fmt)
                .map_err(|e| Error::Backend(e.to_string()))?;
        }
        report(progress, total, total, "done");
        Ok(())
    }

    fn format_ext(fmt: Format) -> &'static str {
        match fmt {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::WebP => "webp",
            _ => "png",
        }
    }

    fn image_format(fmt: Format) -> Result<ImageFormat> {
        match fmt {
            Format::Png => Ok(ImageFormat::Png),
            Format::Jpeg => Ok(ImageFormat::Jpeg),
            Format::WebP => Ok(ImageFormat::WebP),
            other => Err(Error::InvalidInput(format!(
                "unsupported image format {other:?}"
            ))),
        }
    }

    fn report(progress: &Option<ProgressSink>, done: u64, total: u64, stage: &str) {
        if let Some(sink) = progress {
            sink(docsuite_core::Progress::new(done, total, stage.to_owned()));
        }
    }

    fn bind_pdfium() -> Result<Pdfium> {
        use std::env;
        use std::path::PathBuf;
        let name = Pdfium::pdfium_platform_library_name();
        if let Ok(exe) = env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let path = dir.join(&name);
            if path.is_file() {
                return Pdfium::bind_to_library(&path)
                    .map(Pdfium::new)
                    .map_err(|e| Error::Backend(e.to_string()));
            }
        }
        if let Ok(dir) = env::var("DOCAPP_PDFIUM") {
            let path = PathBuf::from(dir).join(&name);
            if path.is_file() {
                return Pdfium::bind_to_library(&path)
                    .map(Pdfium::new)
                    .map_err(|e| Error::Backend(e.to_string()));
            }
            return Err(Error::Backend(format!(
                "DOCAPP_PDFIUM set but {} not found",
                path.display()
            )));
        }
        Pdfium::bind_to_system_library()
            .map(Pdfium::new)
            .map_err(|e| Error::Backend(format!("pdfium missing: {e}")))
    }
}

#[cfg(feature = "pdfium")]
pub use imp::{PdfToImageOptions, pdf_to_image};
