use std::path::Path;
use std::sync::Arc;

use docsuite_core::throttle_progress;
use docsuite_core::{CancelToken, Error, JobRunner, Priority, ProgressSink, Result, ThrottleState};
use docsuite_pdf::Pdf;

use crate::PdfToWordOptions;
use crate::docx::write_docx;
use crate::extract::{collect_fonts, extract_page};
use crate::ocr_util::apply_ocr_to_scanned_page;
use crate::types::ExtractedPage;

pub fn pdf_to_docx(
    input: &Path,
    output: &Path,
    opts: &PdfToWordOptions,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let pdf = Pdf::load(input)?;
    let total = pdf.page_count();
    let page_list = match &opts.pages {
        Some(r) => r.pages(total)?,
        None => (1..=total).collect(),
    };
    if page_list.is_empty() {
        return Err(Error::InvalidInput("no pages selected".to_owned()));
    }
    let doc = pdf.lopdf_document();
    let mut pages = Vec::with_capacity(page_list.len());
    let mut state = ThrottleState::new();
    for (i, &pn) in page_list.iter().enumerate() {
        cancel.check()?;
        throttle_progress(
            progress,
            &mut state,
            docsuite_core::Progress::stage_static(i as u64, page_list.len() as u64, "pdf-to-docx"),
        );
        let page_id = pdf.page_id(pn)?;
        let extracted = extract_page(doc, page_id)?;
        pages.push(extracted);
    }

    if opts.ocr_scanned {
        let ocr = opts
            .page_ocr
            .as_ref()
            .ok_or_else(|| Error::Ocr("ocr not configured".to_owned()))?;
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
        for handle in handles {
            cancel.check()?;
            let (idx, runs) = handle.wait()?;
            apply_ocr_to_scanned_page(&mut pages[idx], runs, true);
        }
    }

    let page_ids: Vec<_> = page_list
        .iter()
        .filter_map(|p| pdf.page_id(*p).ok())
        .collect();
    let fonts = if opts.embed_fonts {
        collect_fonts(&pages, doc, &page_ids)
    } else {
        Vec::new()
    };
    write_docx(output, &pages, opts.mode, opts.embed_fonts, &fonts)?;
    throttle_progress(
        progress,
        &mut state,
        docsuite_core::Progress::stage_static(
            page_list.len() as u64,
            page_list.len() as u64,
            "pdf-to-docx",
        ),
    );
    Ok(())
}

pub fn is_scanned(page: &ExtractedPage) -> bool {
    if !page.text.is_empty() {
        return false;
    }
    if page.images.len() != 1 {
        return false;
    }
    let img = &page.images[0];
    let area = img.width * img.height;
    let page_area = page.width * page.height;
    page_area > 0.0 && area / page_area > 0.8
}
