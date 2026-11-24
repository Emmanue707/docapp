use std::io::Write;
use std::path::Path;

use docsuite_core::throttle_progress;
use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};
use docsuite_pdf::{Pdf, PlacedWord, add_searchable_text_layer};

use crate::extract::extract_page;
use crate::ocr_util::{lines_plain_text, runs_to_text_lines};
use crate::pdf_word::is_scanned;

pub struct PdfTextOptions {
    pub pages: Option<docsuite_pdf::PageRanges>,
    pub ocr: bool,
    pub page_ocr: Option<std::sync::Arc<dyn crate::PageOcr>>,
}

fn page_plain_from_extract(page: &crate::types::ExtractedPage) -> String {
    let mut out = String::new();
    for line in &page.text {
        for (i, run) in line.runs.iter().enumerate() {
            if i > 0 {
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

fn runs_to_placed(runs: &[crate::types::TextRun]) -> Vec<PlacedWord> {
    runs.iter()
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
        .collect()
}

pub fn pdf_to_txt(
    input: &Path,
    output: &Path,
    opts: &PdfTextOptions,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let pdf = Pdf::load(input)?;
    let total = pdf.page_count();
    let page_list = match &opts.pages {
        Some(r) => r.pages(total)?,
        None => (1..=total).collect(),
    };
    let doc = pdf.lopdf_document();
    let mut state = ThrottleState::new();
    let mut all = String::new();
    for (i, &pn) in page_list.iter().enumerate() {
        cancel.check()?;
        throttle_progress(
            progress,
            &mut state,
            docsuite_core::Progress::stage_static(i as u64, page_list.len() as u64, "pdf-to-txt"),
        );
        let page_id = pdf.page_id(pn)?;
        let extracted = extract_page(doc, page_id)?;
        if extracted.text.is_empty() && opts.ocr && is_scanned(&extracted) {
            let ocr = opts
                .page_ocr
                .as_ref()
                .ok_or_else(|| Error::Ocr("ocr not configured".to_owned()))?;
            if let Some(img) = extracted.images.first() {
                let runs = ocr.ocr_placed(img, cancel)?;
                all.push_str(&runs_plain(&runs));
            }
        } else {
            all.push_str(&page_plain_from_extract(&extracted));
        }
        if i + 1 < page_list.len() && !all.ends_with('\n') {
            all.push('\n');
        }
    }
    let mut file = std::fs::File::create(output).map_err(Error::from)?;
    file.write_all(all.as_bytes()).map_err(Error::from)?;
    Ok(())
}

pub fn pdf_to_searchable_pdf(
    input: &Path,
    output: &Path,
    opts: &PdfTextOptions,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let mut pdf = Pdf::load(input)?;
    let total = pdf.page_count();
    let page_list = match &opts.pages {
        Some(r) => r.pages(total)?,
        None => (1..=total).collect(),
    };
    let in_doc = Pdf::load(input)?;
    let doc = in_doc.lopdf_document();
    let mut state = ThrottleState::new();
    for (i, &pn) in page_list.iter().enumerate() {
        cancel.check()?;
        throttle_progress(
            progress,
            &mut state,
            docsuite_core::Progress::stage_static(
                i as u64,
                page_list.len() as u64,
                "searchable-pdf",
            ),
        );
        let page_id = in_doc.page_id(pn)?;
        let extracted = extract_page(doc, page_id)?;
        let mut words = Vec::new();
        for line in &extracted.text {
            for run in &line.runs {
                words.extend(runs_to_placed(std::slice::from_ref(run)));
            }
        }
        if words.is_empty() && opts.ocr && is_scanned(&extracted) {
            let ocr = opts
                .page_ocr
                .as_ref()
                .ok_or_else(|| Error::Ocr("ocr not configured".to_owned()))?;
            if let Some(img) = extracted.images.first() {
                let runs = ocr.ocr_placed(img, cancel)?;
                words = runs_to_placed(&runs);
            }
        }
        if !words.is_empty() {
            add_searchable_text_layer(&mut pdf, pn, &words, cancel, progress)?;
        }
    }
    pdf.save(output)?;
    Ok(())
}
