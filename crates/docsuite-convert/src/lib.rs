use std::fs;
use std::io::Read;
use std::path::Path;

use docsuite_core::{CancelToken, Error, ProgressSink, Result};
use docsuite_pdf::{ImagePageFit, PageRanges, pdf_from_image_file};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PdfToWordMode {
    #[default]
    ExactLook,
    Editable,
}

#[derive(Clone, Debug, Default)]
pub struct PdfToWordOptions {
    pub mode: PdfToWordMode,
    pub embed_fonts: bool,
    pub ocr_scanned: bool,
    pub pages: Option<PageRanges>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Docx,
    Odt,
    Jpeg,
    Png,
}

#[derive(Clone, Debug, Default)]
pub struct ImageToPdfOptions {
    pub fit: ImagePageFit,
}

#[derive(Clone, Debug, Default)]
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

pub fn pdf_to_docx(
    _input: &Path,
    _output: &Path,
    _opts: &PdfToWordOptions,
    _cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    Err(Error::Unsupported("pdf_to_docx not implemented".to_owned()))
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

pub fn pdf_to_image(
    _input: &Path,
    _output: &Path,
    _cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    Err(Error::Unsupported(
        "pdf_to_image not implemented".to_owned(),
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
    match (input_fmt, output_fmt) {
        (Format::Jpeg | Format::Png, Format::Pdf) => {
            image_to_pdf(input, output, &opts.image, cancel, progress)
        }
        (Format::Pdf, Format::Docx) => {
            pdf_to_docx(input, output, &opts.pdf_to_word, cancel, progress)
        }
        (Format::Odt, Format::Docx) => odt_to_docx(input, output, cancel, progress),
        (Format::Docx, Format::Odt) => docx_to_odt(input, output, cancel, progress),
        (Format::Docx, Format::Pdf) => docx_to_pdf(input, output, cancel, progress),
        (Format::Odt, Format::Pdf) => odt_to_pdf(input, output, cancel, progress),
        (Format::Pdf, Format::Jpeg | Format::Png) => pdf_to_image(input, output, cancel, progress),
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
