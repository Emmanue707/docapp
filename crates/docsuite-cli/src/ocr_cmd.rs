use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use docsuite_convert::{
    Format, ImageOcrOptions, MAX_OCR_LONG_SIDE, PdfTextOptions, PdfToWordMode, RecognizerPageOcr,
    cap_long_side, decode_image_gray, detect_format, image_to_docx, image_to_searchable_pdf,
    image_to_txt, pdf_to_searchable_pdf, pdf_to_txt,
};
use docsuite_core::{CancelToken, ProgressSink};
use docsuite_ocr::{BBox, open_recognizer, predict_type};
use docsuite_pdf::PageRanges;

use crate::models::models_dir;
use crate::util::{field_type_label, parse_rect, progress_sink};

pub struct OcrRunOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub pages: Option<String>,
    pub region: Option<String>,
    pub types: bool,
    pub handwriting: bool,
    pub models_dir: PathBuf,
    pub quiet: bool,
}

fn build_ocr(dir: &Path, handwriting: bool) -> Result<Arc<RecognizerPageOcr>> {
    let rec = open_recognizer(dir, handwriting).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(Arc::new(RecognizerPageOcr {
        recognizer: rec,
        highlight_low_confidence: false,
    }))
}

fn region_bbox(spec: &str) -> Result<BBox> {
    let r = parse_rect(spec)?;
    Ok(BBox {
        x0: r.llx as f32,
        y0: r.lly as f32,
        x1: r.urx as f32,
        y1: r.ury as f32,
    })
}

pub fn cmd_ocr(opts: OcrRunOptions) -> Result<()> {
    let cancel = CancelToken::new();
    let progress = progress_sink(opts.quiet);
    let ocr = build_ocr(&opts.models_dir, opts.handwriting)?;
    let fmt = detect_format(&opts.input).map_err(|e| anyhow::anyhow!("{e}"))?;
    match fmt {
        Format::Pdf => run_pdf(&opts, ocr, &cancel, &progress),
        Format::Jpeg | Format::Png | Format::WebP => run_image(&opts, ocr, &cancel, &progress),
        _ => bail!("ocr input must be pdf or image"),
    }
}

fn run_image(
    opts: &OcrRunOptions,
    ocr: Arc<RecognizerPageOcr>,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    if let Some(out) = &opts.output {
        match out.extension().and_then(|e| e.to_str()) {
            Some("txt") => image_to_txt(&opts.input, out, ocr.as_ref(), cancel, progress)
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            Some("docx") => image_to_docx(
                &opts.input,
                out,
                &ImageOcrOptions {
                    mode: PdfToWordMode::Editable,
                    ocr: ocr.clone(),
                },
                cancel,
                progress,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?,
            Some("pdf") => {
                image_to_searchable_pdf(&opts.input, out, ocr.as_ref(), cancel, progress)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
            }
            _ => bail!("unsupported output extension"),
        }
        return Ok(());
    }
    let bytes = std::fs::read(&opts.input).context("read input")?;
    let gray = decode_image_gray(&bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    let (scaled, _) = cap_long_side(gray, MAX_OCR_LONG_SIDE);
    let region = opts.region.as_deref().map(region_bbox).transpose()?;
    let lines = ocr
        .recognizer
        .recognize(&scaled, region, cancel)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let text = docsuite_convert::lines_plain_text(&lines);
    if opts.types {
        for word in text.split_whitespace() {
            let guess = predict_type(word);
            println!("{} {}", word, field_type_label(guess.field_type));
        }
    } else {
        print!("{text}");
    }
    Ok(())
}

fn run_pdf(
    opts: &OcrRunOptions,
    ocr: Arc<RecognizerPageOcr>,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let pages = match &opts.pages {
        Some(p) => Some(PageRanges::parse(p).map_err(|e| anyhow::anyhow!("{e}"))?),
        None => None,
    };
    let text_opts = PdfTextOptions {
        pages,
        ocr: true,
        page_ocr: Some(ocr.clone()),
    };
    if let Some(out) = &opts.output {
        match out.extension().and_then(|e| e.to_str()) {
            Some("txt") => pdf_to_txt(&opts.input, out, &text_opts, cancel, progress)
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            Some("pdf") => pdf_to_searchable_pdf(&opts.input, out, &text_opts, cancel, progress)
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            Some("docx") => bail!("use convert for pdf to docx"),
            _ => bail!("unsupported output extension"),
        }
        return Ok(());
    }
    let tmp = std::env::temp_dir().join(format!("docapp-ocr-out-{}.txt", std::process::id()));
    pdf_to_txt(&opts.input, &tmp, &text_opts, cancel, progress)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let text = std::fs::read_to_string(&tmp).context("read ocr output")?;
    let _ = std::fs::remove_file(&tmp);
    if opts.types {
        for word in text.split_whitespace() {
            let guess = predict_type(word);
            println!("{} {}", word, field_type_label(guess.field_type));
        }
    } else {
        print!("{text}");
    }
    Ok(())
}

pub fn default_models_dir(custom: Option<PathBuf>) -> PathBuf {
    models_dir(custom)
}
