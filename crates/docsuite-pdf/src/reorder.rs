use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};
use lopdf::Object;

use crate::lopdf_map::map_err;
use crate::pdf::Pdf;
use crate::progress::step;
use crate::prune::finalize_doc;

fn page_root_id(pdf: &Pdf) -> Result<lopdf::ObjectId> {
    let catalog = pdf.document().catalog().map_err(map_err)?;
    catalog
        .get(b"Pages")
        .and_then(Object::as_reference)
        .map_err(map_err)
}

pub fn reorder(
    pdf: &mut Pdf,
    order: &[u32],
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let total = pdf.page_count();
    if order.is_empty() {
        return Err(Error::InvalidInput("empty page order".to_owned()));
    }
    let pages = pdf.document().get_pages();
    let mut seen = std::collections::HashSet::new();
    for &page in order {
        if page == 0 || page > total {
            return Err(Error::InvalidInput(format!("page {page} out of range")));
        }
        if !seen.insert(page) {
            return Err(Error::InvalidInput(format!("duplicate page {page}")));
        }
    }
    if order.len() != total as usize {
        return Err(Error::InvalidInput(
            "order must list every page once".to_owned(),
        ));
    }
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "reorder")?;
    let root = page_root_id(pdf)?;
    let kids: Vec<Object> = order
        .iter()
        .map(|p| {
            let id = pages
                .get(p)
                .copied()
                .ok_or_else(|| Error::InvalidInput(format!("page {p} not found")))?;
            Ok(Object::Reference(id))
        })
        .collect::<Result<_>>()?;
    let root_mut = pdf
        .document_mut()
        .get_dictionary_mut(root)
        .map_err(map_err)?;
    root_mut.set("Kids", Object::Array(kids));
    root_mut.set("Count", Object::Integer(order.len() as i64));
    Ok(())
}

pub fn delete_pages(
    pdf: &mut Pdf,
    pages: &[u32],
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let total = pdf.page_count();
    let mut state = ThrottleState::new();
    for (i, page) in pages.iter().enumerate() {
        step(
            cancel,
            progress,
            &mut state,
            i as u64,
            pages.len() as u64,
            "delete",
        )?;
        if *page == 0 || *page > total {
            return Err(Error::InvalidInput(format!("page {page} out of range")));
        }
    }
    pdf.document_mut().delete_pages(pages);
    finalize_doc(pdf.document_mut());
    Ok(())
}

pub fn extract_pages(
    pdf: &Pdf,
    pages: &[u32],
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<Pdf> {
    let total = pdf.page_count();
    for &page in pages {
        if page == 0 || page > total {
            return Err(Error::InvalidInput(format!("page {page} out of range")));
        }
    }
    let keep: std::collections::HashSet<u32> = pages.iter().copied().collect();
    let delete: Vec<u32> = (1..=total).filter(|p| !keep.contains(p)).collect();
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "extract")?;
    let mut doc = pdf.document().clone();
    if !delete.is_empty() {
        doc.delete_pages(&delete);
    }
    finalize_doc(&mut doc);
    Ok(Pdf::from_document(doc))
}
