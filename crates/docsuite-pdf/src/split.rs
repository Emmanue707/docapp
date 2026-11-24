use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};

use crate::pdf::Pdf;
use crate::progress::step;
use crate::prune::finalize_doc;
use crate::ranges::PageRanges;

fn retain_pages(pdf: &Pdf, keep: &[u32]) -> Result<Pdf> {
    let total = pdf.page_count();
    if total == 0 {
        return Err(Error::InvalidInput("document has no pages".to_owned()));
    }
    let keep_set: std::collections::HashSet<u32> = keep.iter().copied().collect();
    let delete: Vec<u32> = (1..=total).filter(|p| !keep_set.contains(p)).collect();
    let mut doc = pdf.document().clone();
    if !delete.is_empty() {
        doc.delete_pages(&delete);
    }
    finalize_doc(&mut doc);
    Ok(Pdf::from_document(doc))
}

pub fn split(
    pdf: &Pdf,
    ranges: &[PageRanges],
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<Vec<Pdf>> {
    let total = pdf.page_count();
    let mut out = Vec::with_capacity(ranges.len());
    let mut state = ThrottleState::new();
    for (i, range) in ranges.iter().enumerate() {
        step(
            cancel,
            progress,
            &mut state,
            i as u64,
            ranges.len() as u64,
            "split",
        )?;
        let keep = range.pages(total)?;
        out.push(retain_pages(pdf, &keep)?);
    }
    Ok(out)
}

pub fn split_every(
    pdf: &Pdf,
    n: u32,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<Vec<Pdf>> {
    if n == 0 {
        return Err(Error::InvalidInput(
            "split size must be positive".to_owned(),
        ));
    }
    let total = pdf.page_count();
    let mut ranges = Vec::new();
    let mut start = 1u32;
    while start <= total {
        let end = (start + n - 1).min(total);
        ranges.push(PageRanges::from_range(start, Some(end)));
        start = end + 1;
    }
    split(pdf, &ranges, cancel, progress)
}
