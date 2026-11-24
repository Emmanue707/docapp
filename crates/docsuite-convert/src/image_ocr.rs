use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use docsuite_core::{CancelToken, Error, ProgressSink, Result};
use docsuite_pdf::{ImagePageFit, PlacedWord, add_searchable_text_layer, pdf_from_image_file};

use crate::PdfToWordMode;
use crate::docx::write_docx;
use crate::ocr_util::{MAX_OCR_LONG_SIDE, cap_long_side, decode_image_gray, runs_to_text_lines};
use crate::types::{ExtractedPage, ImageKind, PlacedImage};

pub struct ImageOcrOptions {
    pub mode: PdfToWordMode,
    pub ocr: Arc<dyn crate::PageOcr>,
}

fn runs_plain(runs: &[crate::types::TextRun]) -> String {
    let mut out = String::new();
    for line in runs_to_text_lines(runs) {
        for (ri, run) in line.runs.iter().enumerate() {
            if ri > 0 {
                out.push(' ');
            }
            out.push_str(&run.text);
        }
        if !line.runs.is_empty() {
            out.push('\n');
        }
    }
    out
}

pub fn image_to_txt(
    input: &Path,
    output: &Path,
    ocr: &dyn crate::PageOcr,
    cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    let img = load_placed(input)?;
    let runs = ocr.ocr_placed(&img, cancel)?;
    let text = runs_plain(&runs);
    let mut file = std::fs::File::create(output).map_err(Error::from)?;
    file.write_all(text.as_bytes()).map_err(Error::from)?;
    Ok(())
}

pub fn image_to_docx(
    input: &Path,
    output: &Path,
    opts: &ImageOcrOptions,
    cancel: &CancelToken,
    _progress: &Option<ProgressSink>,
) -> Result<()> {
    let bytes = std::fs::read(input).map_err(Error::from)?;
    let kind = sniff_image_kind(&bytes);
    let img = load_placed(input)?;
    let runs = opts.ocr.ocr_placed(&img, cancel)?;
    let w_pt = img.width;
    let h_pt = img.height;
    let page = ExtractedPage {
        width: w_pt,
        height: h_pt,
        rotate: 0,
        text: runs_to_text_lines(&runs),
        images: vec![PlacedImage {
            bytes,
            kind,
            x: 0.0,
            y: 0.0,
            width: w_pt,
            height: h_pt,
            behind: true,
        }],
        vectors: Vec::new(),
    };
    write_docx(output, &[page], opts.mode, false, &[])?;
    Ok(())
}

pub fn image_to_searchable_pdf(
    input: &Path,
    output: &Path,
    ocr: &dyn crate::PageOcr,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let img = load_placed(input)?;
    let runs = ocr.ocr_placed(&img, cancel)?;
    let mut pdf = pdf_from_image_file(input, ImagePageFit::Natural72, cancel, progress)?;
    let words: Vec<PlacedWord> = runs
        .iter()
        .map(|run| {
            let h = run.font_size;
            PlacedWord {
                text: run.text.clone(),
                x: run.x,
                y: run.y + h * 0.85,
                width: run.width.max(1.0),
                height: h,
            }
        })
        .collect();
    if !words.is_empty() {
        add_searchable_text_layer(&mut pdf, 1, &words, cancel, progress)?;
    }
    pdf.save(output)?;
    Ok(())
}

fn load_placed(input: &Path) -> Result<PlacedImage> {
    let bytes = std::fs::read(input).map_err(Error::from)?;
    let kind = sniff_image_kind(&bytes);
    let gray = decode_image_gray(&bytes)?;
    let (scaled, _) = cap_long_side(gray, MAX_OCR_LONG_SIDE);
    Ok(PlacedImage {
        bytes,
        kind,
        x: 0.0,
        y: 0.0,
        width: scaled.width as f64,
        height: scaled.height as f64,
        behind: false,
    })
}

fn sniff_image_kind(bytes: &[u8]) -> ImageKind {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        ImageKind::Jpeg
    } else {
        ImageKind::Png
    }
}
