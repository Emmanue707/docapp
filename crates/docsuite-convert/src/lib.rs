#![allow(clippy::collapsible_if, unused_extern_crates, unused_imports)]

#[macro_use]
extern crate lopdf;

mod docx;
pub mod extract;
mod fonts;
mod image_ocr;
mod matrix;
mod obfuscate;
mod ocr_util;
mod page_ocr;
#[cfg(feature = "pdfium")]
mod pdf_image;
mod pdf_text;
mod pdf_word;
mod types;

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use docsuite_core::{CancelToken, Error, ProgressSink, Result};
use docsuite_pdf::{ImagePageFit, PageRanges, pdf_from_image_file};

pub use image_ocr::{ImageOcrOptions, image_to_docx, image_to_searchable_pdf, image_to_txt};
pub use pdf_text::{PdfTextOptions, pdf_to_searchable_pdf, pdf_to_txt};
pub use pdf_word::pdf_to_docx;
pub use types::{ExtractedPage, TextRun};

pub trait PageOcr: Send + Sync {
    fn ocr_page(
        &self,
        image: &[u8],
        width: u32,
        height: u32,
        cancel: &CancelToken,
    ) -> Result<Vec<TextRun>>;

    fn ocr_placed(&self, img: &types::PlacedImage, cancel: &CancelToken) -> Result<Vec<TextRun>> {
        let w = img.width.max(1.0) as u32;
        let h = img.height.max(1.0) as u32;
        self.ocr_page(&img.bytes, w, h, cancel)
    }
}

pub use ocr_util::{MAX_OCR_LONG_SIDE, cap_long_side, decode_image_gray, lines_plain_text};
pub use page_ocr::RecognizerPageOcr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PdfToWordMode {
    #[default]
    ExactLook,
    Editable,
}

#[derive(Clone, Default)]
pub struct PdfToWordOptions {
    pub mode: PdfToWordMode,
    pub embed_fonts: bool,
    pub ocr_scanned: bool,
    pub highlight_low_confidence: bool,
    pub pages: Option<PageRanges>,
    pub page_ocr: Option<Arc<dyn PageOcr>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Docx,
    Odt,
    Jpeg,
    Png,
    Txt,
    WebP,
}

#[derive(Clone, Debug, Default)]
pub struct ImageToPdfOptions {
    pub fit: ImagePageFit,
}

#[derive(Clone, Default)]
pub struct ConvertOptions {
    pub image: ImageToPdfOptions,
    pub pdf_to_word: PdfToWordOptions,
}

fn sniff_format(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"%PDF") {
        return Some(Format::Pdf);
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return Some(Format::Jpeg);
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some(Format::Png);
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(Format::WebP);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return None;
    }
    None
}

fn format_from_extension(path: &Path) -> Option<Format> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pdf") => Some(Format::Pdf),
        Some("docx") => Some(Format::Docx),
        Some("odt") => Some(Format::Odt),
        Some("jpg") | Some("jpeg") => Some(Format::Jpeg),
        Some("png") => Some(Format::Png),
        Some("webp") => Some(Format::WebP),
        Some("txt") => Some(Format::Txt),
        _ => None,
    }
}

pub fn detect_format(path: &Path) -> Result<Format> {
    let mut file = fs::File::open(path).map_err(Error::from)?;
    let mut header = [0u8; 8];
    let read = file.read(&mut header).map_err(Error::from)?;
    if let Some(fmt) = sniff_format(&header[..read]) {
        return Ok(fmt);
    }
    if header.starts_with(b"PK\x03\x04") {
        return format_from_extension(path).ok_or_else(|| {
            Error::InvalidInput("zip container requires known extension".to_owned())
        });
    }
    format_from_extension(path).ok_or_else(|| {
        Error::InvalidInput(format!(
            "unknown format for {}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
        ))
    })
}

pub fn odt_to_docx(
    _input: &Path,
    _output: &Path,
    _cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    Err(Error::Unsupported("odt_to_docx not implemented".to_owned()))
}

pub fn docx_to_odt(
    _input: &Path,
    _output: &Path,
    _cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    Err(Error::Unsupported("docx_to_odt not implemented".to_owned()))
}

pub fn docx_to_pdf(
    _input: &Path,
    _output: &Path,
    _cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    Err(Error::Unsupported("docx_to_pdf not implemented".to_owned()))
}

pub fn odt_to_pdf(
    _input: &Path,
    _output: &Path,
    _cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    Err(Error::Unsupported("odt_to_pdf not implemented".to_owned()))
}

#[cfg(feature = "pdfium")]
pub use pdf_image::{PdfToImageOptions, pdf_to_image};

#[cfg(not(feature = "pdfium"))]
pub fn pdf_to_image(
    _input: &Path,
    _output: &Path,
    _cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    Err(Error::Unsupported(
        "pdf_to_image requires the pdfium feature".to_owned(),
    ))
}

pub fn image_to_pdf(
    input: &Path,
    output: &Path,
    opts: &ImageToPdfOptions,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let mut pdf = pdf_from_image_file(input, opts.fit, cancel, progress)?;
    pdf.save(output)?;
    Ok(())
}

fn format_for_output(path: &Path) -> Result<Format> {
    format_from_extension(path).ok_or_else(|| {
        Error::InvalidInput(format!(
            "unknown output format for {}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
        ))
    })
}

pub fn convert(
    input: &Path,
    output: &Path,
    opts: &ConvertOptions,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let input_fmt = detect_format(input)?;
    let output_fmt = format_for_output(output)?;
    let txt_opts = PdfTextOptions {
        pages: opts.pdf_to_word.pages.clone(),
        ocr: opts.pdf_to_word.ocr_scanned,
        page_ocr: opts.pdf_to_word.page_ocr.clone(),
    };
    let img_ocr = opts.pdf_to_word.page_ocr.clone();
    match (input_fmt, output_fmt) {
        (Format::Jpeg | Format::Png | Format::WebP, Format::Pdf) => {
            if opts.pdf_to_word.ocr_scanned {
                let ocr = img_ocr.ok_or_else(|| Error::Ocr("ocr not configured".to_owned()))?;
                image_to_searchable_pdf(input, output, ocr.as_ref(), cancel, progress)
            } else {
                image_to_pdf(input, output, &opts.image, cancel, progress)
            }
        }
        (Format::Jpeg | Format::Png | Format::WebP, Format::Docx) => {
            let ocr = img_ocr.ok_or_else(|| Error::Ocr("ocr not configured".to_owned()))?;
            image_to_docx(
                input,
                output,
                &ImageOcrOptions {
                    mode: opts.pdf_to_word.mode,
                    ocr,
                },
                cancel,
                progress,
            )
        }
        (Format::Jpeg | Format::Png | Format::WebP, Format::Txt) => {
            let ocr = img_ocr.ok_or_else(|| Error::Ocr("ocr not configured".to_owned()))?;
            image_to_txt(input, output, ocr.as_ref(), cancel, progress)
        }
        (Format::Pdf, Format::Docx) => {
            pdf_to_docx(input, output, &opts.pdf_to_word, cancel, progress)
        }
        (Format::Pdf, Format::Txt) => pdf_to_txt(input, output, &txt_opts, cancel, progress),
        (Format::Pdf, Format::Pdf) => {
            if opts.pdf_to_word.ocr_scanned {
                pdf_to_searchable_pdf(input, output, &txt_opts, cancel, progress)
            } else {
                Err(Error::Unsupported("pdf to pdf needs --ocr".to_owned()))
            }
        }
        (Format::Odt, Format::Docx) => odt_to_docx(input, output, cancel, progress),
        (Format::Docx, Format::Odt) => docx_to_odt(input, output, cancel, progress),
        (Format::Docx, Format::Pdf) => docx_to_pdf(input, output, cancel, progress),
        (Format::Odt, Format::Pdf) => odt_to_pdf(input, output, cancel, progress),
        (Format::Pdf, Format::Jpeg | Format::Png | Format::WebP) => {
            #[cfg(feature = "pdfium")]
            {
                let opts = PdfToImageOptions {
                    dpi: 150.0,
                    pages: opts.pdf_to_word.pages.clone(),
                    format: output_fmt,
                };
                pdf_to_image(input, output, &opts, cancel, progress)
            }
            #[cfg(not(feature = "pdfium"))]
            {
                let _ = output_fmt;
                pdf_to_image(input, output, cancel, progress)
            }
        }
        _ => Err(Error::Unsupported(format!(
            "conversion {:?} -> {:?} is not supported",
            input_fmt, output_fmt
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docsuite_core::CancelToken;
    use std::io::Write;

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn detect_png_and_convert() {
        let dir = std::env::temp_dir().join(format!("docsuite-convert-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let png_path = dir.join("in.png");
        let pdf_path = dir.join("out.pdf");
        let mut f = std::fs::File::create(&png_path).unwrap();
        f.write_all(&tiny_png()).unwrap();
        assert_eq!(detect_format(&png_path).unwrap(), Format::Png);
        let cancel = CancelToken::new();
        image_to_pdf(
            &png_path,
            &pdf_path,
            &ImageToPdfOptions::default(),
            &cancel,
            &None,
        )
        .unwrap();
        assert!(pdf_path.exists());
        assert_eq!(detect_format(&pdf_path).unwrap(), Format::Pdf);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
#[path = "pdf_docx_tests.rs"]
mod pdf_docx_tests;

#[cfg(test)]
#[path = "ocr_tests.rs"]
mod ocr_tests;
