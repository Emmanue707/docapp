use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use docsuite_core::throttle_progress;
use docsuite_core::{CancelToken, Error, JobRunner, Priority, ProgressSink, Result, ThrottleState};
use docsuite_pdf::page_attr::mediabox_array;
use docsuite_pdf::{Pdf, PlacedWord, add_searchable_text_layer};

use crate::extract::extract_page;
use crate::ocr_util::{lines_plain_text, runs_to_text_lines};
use crate::page_rotate::{display_run_to_user_space, page_size_user};
use crate::pdf_word::is_scanned;
use crate::types::{ExtractedPage, TextRun};

pub struct PdfTextOptions {
    pub pages: Option<docsuite_pdf::PageRanges>,
    pub ocr: bool,
    pub page_ocr: Option<Arc<dyn crate::PageOcr>>,
}

fn page_plain_from_extract(page: &ExtractedPage) -> String {
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

fn runs_plain(runs: &[TextRun]) -> String {
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

fn runs_to_placed(runs: &[TextRun], page: &ExtractedPage) -> Vec<PlacedWord> {
    runs.iter()
        .map(|run| {
            let user = display_run_to_user_space(run, page);
            let h = user.font_size;
            PlacedWord {
                text: user.text.clone(),
                x: user.x,
                y: user.y + h * 0.85,
                width: user.width.max(1.0),
                height: h,
            }
        })
        .collect()
}

fn user_page_height(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> f64 {
    let box4 = mediabox_array(doc, page_id).unwrap_or([0.0, 0.0, 612.0, 792.0]);
    page_size_user(box4).1
}

fn parallel_scanned_ocr(
    pages: &mut [ExtractedPage],
    opts: &PdfTextOptions,
    cancel: &CancelToken,
) -> Result<Vec<Option<Vec<TextRun>>>> {
    let Some(ocr) = opts.page_ocr.as_ref() else {
        return Ok(vec![None; pages.len()]);
    };
    if !opts.ocr {
        return Ok(vec![None; pages.len()]);
    }
    let runner = JobRunner::builder()
        .workers(2)
        .queue_capacity(pages.len())
        .build();
    let mut handles = Vec::new();
    for (idx, page) in pages.iter().enumerate() {
        if !is_scanned(page) {
            continue;
        }
        let Some(img) = page.images.first().cloned() else {
            continue;
        };
        let ocr = Arc::clone(ocr);
        let cancel = cancel.clone();
        let handle = runner.submit(Priority::Normal, move |_ctx| {
            cancel.check()?;
            let runs = ocr.ocr_placed(&img, &cancel)?;
            Ok((idx, runs))
        });
        handles.push(handle);
    }
    let mut out: Vec<Option<Vec<TextRun>>> = vec![None; pages.len()];
    for handle in handles {
        cancel.check()?;
        let (idx, runs) = handle.wait()?;
        out[idx] = Some(runs);
    }
    Ok(out)
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
    let mut pages = Vec::with_capacity(page_list.len());
    let mut state = ThrottleState::new();
    for (i, &pn) in page_list.iter().enumerate() {
        cancel.check()?;
        throttle_progress(
            progress,
            &mut state,
            docsuite_core::Progress::stage_static(i as u64, page_list.len() as u64, "pdf-to-txt"),
        );
        let page_id = pdf.page_id(pn)?;
        pages.push(extract_page(doc, page_id)?);
    }
    let ocr_runs = parallel_scanned_ocr(&mut pages, opts, cancel)?;
    let mut all = String::new();
    for (i, page) in pages.iter().enumerate() {
        if let Some(runs) = &ocr_runs[i] {
            all.push_str(&runs_plain(runs));
        } else {
            all.push_str(&page_plain_from_extract(page));
        }
        if i + 1 < pages.len() && !all.ends_with('\n') {
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
    let mut pages = Vec::with_capacity(page_list.len());
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
        pages.push(extract_page(doc, page_id)?);
    }
    let ocr_runs = parallel_scanned_ocr(&mut pages, opts, cancel)?;
    for (i, &pn) in page_list.iter().enumerate() {
        cancel.check()?;
        let page_id = in_doc.page_id(pn)?;
        let extracted = &pages[i];
        let mut words = Vec::new();
        for line in &extracted.text {
            for run in &line.runs {
                words.extend(runs_to_placed(std::slice::from_ref(run), extracted));
            }
        }
        if let Some(runs) = &ocr_runs[i] {
            words = runs_to_placed(runs, extracted);
        }
        if !words.is_empty() {
            let ph = user_page_height(doc, page_id);
            add_searchable_text_layer(&mut pdf, pn, &words, ph, cancel, progress)?;
        }
    }
    pdf.save(output)?;
    Ok(())
}
