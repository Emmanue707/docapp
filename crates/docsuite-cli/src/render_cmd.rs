use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use docsuite_convert::{Format, PdfToImageOptions, detect_format, pdf_to_image};
use docsuite_core::CancelToken;
use docsuite_pdf::PageRanges;

use crate::util::{ensure_output, ensure_output_dir, progress_sink};

pub struct RenderOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub pages: Option<String>,
    pub dpi: f32,
    pub quiet: bool,
    pub force: bool,
}

pub fn cmd_render(opts: RenderOptions) -> Result<()> {
    let fmt = detect_format(&opts.input).context("detect input")?;
    if fmt != Format::Pdf {
        bail!("render expects a PDF input");
    }
    let out_fmt = output_format(&opts.output)?;
    if opts.output.extension().is_some() {
        ensure_output(&opts.output, &[opts.input.as_path()], opts.force)?;
    } else {
        ensure_output_dir(&opts.output, &opts.input, opts.force)?;
    }
    let pages = match &opts.pages {
        Some(p) => Some(PageRanges::parse(p).map_err(|e| anyhow::anyhow!("{e}"))?),
        None => None,
    };
    let cancel = CancelToken::new();
    let progress = progress_sink(opts.quiet);
    let image_opts = PdfToImageOptions {
        dpi: opts.dpi,
        pages,
        format: out_fmt,
    };
    pdf_to_image(&opts.input, &opts.output, &image_opts, &cancel, &progress)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn output_format(path: &Path) -> Result<Format> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok(Format::Png),
        Some("jpg") | Some("jpeg") => Ok(Format::Jpeg),
        Some("webp") => Ok(Format::WebP),
        None => Ok(Format::Png),
        other => bail!("unsupported output extension {:?}", other),
    }
}
