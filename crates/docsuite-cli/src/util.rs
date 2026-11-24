use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use docsuite_core::{Progress, ProgressSink};
use docsuite_ocr::FieldType;
use docsuite_pdf::{PageRanges, PdfRect};

pub fn progress_sink(quiet: bool) -> Option<ProgressSink> {
    if quiet || !std::io::stderr().is_terminal() {
        return None;
    }
    Some(Arc::new(|p: Progress| {
        let _ = writeln!(std::io::stderr(), "{} {}/{}", p.stage, p.done, p.total);
    }))
}

pub fn ensure_output(output: &Path, inputs: &[&Path], force: bool) -> Result<()> {
    let canonical_out = output.canonicalize().ok();
    for input in inputs {
        if *input == output {
            bail!("refusing to overwrite input file (use --force)");
        }
        if let (Some(out), Ok(cin)) = (&canonical_out, input.canonicalize())
            && &cin == out
            && !force
        {
            bail!("refusing to overwrite input file (use --force)");
        }
    }
    if output.exists() && !force {
        bail!("output file exists (use --force)");
    }
    Ok(())
}

pub fn ensure_output_dir(output_dir: &Path, _input: &Path, force: bool) -> Result<()> {
    if output_dir.exists() && !output_dir.is_dir() {
        bail!("output path is not a directory");
    }
    if !force && output_dir.exists() {
        let entries = std::fs::read_dir(output_dir).context("read output dir")?;
        if entries.filter_map(|e| e.ok()).any(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
        }) {
            bail!("output directory already contains PDF files (use --force)");
        }
    }
    std::fs::create_dir_all(output_dir).context("create output dir")?;
    Ok(())
}

pub fn parse_pages(spec: &str, page_count: u32) -> Result<Vec<u32>> {
    PageRanges::parse(spec)
        .context("parse pages")?
        .pages(page_count)
        .map_err(Into::into)
}

pub fn parse_rect(spec: &str) -> Result<PdfRect> {
    let mut parts = spec.split(',');
    let x: f64 = parts
        .next()
        .context("rect x")?
        .trim()
        .parse()
        .context("rect x")?;
    let y: f64 = parts
        .next()
        .context("rect y")?
        .trim()
        .parse()
        .context("rect y")?;
    let w: f64 = parts
        .next()
        .context("rect width")?
        .trim()
        .parse()
        .context("rect width")?;
    let h: f64 = parts
        .next()
        .context("rect height")?
        .trim()
        .parse()
        .context("rect height")?;
    Ok(PdfRect {
        llx: x,
        lly: y,
        urx: x + w,
        ury: y + h,
    })
}

pub fn parse_order(spec: &str) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let page: u32 = part
            .parse()
            .with_context(|| format!("invalid page '{part}'"))?;
        if page == 0 {
            bail!("page numbers are 1-based");
        }
        out.push(page);
    }
    if out.is_empty() {
        bail!("empty page order");
    }
    Ok(out)
}

pub fn field_type_label(ft: FieldType) -> &'static str {
    match ft {
        FieldType::Date => "date",
        FieldType::Currency => "currency",
        FieldType::CodeId => "code_id",
        FieldType::Email => "email",
        FieldType::Number => "number",
        FieldType::Text => "text",
    }
}

pub fn part_path(dir: &Path, stem: &str, index: usize) -> PathBuf {
    dir.join(format!("{stem}-part{index:03}.pdf"))
}
