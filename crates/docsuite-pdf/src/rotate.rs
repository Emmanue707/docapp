use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};

use crate::lopdf_map::map_err;
use crate::page_attr::resolve_rotate;
use crate::pdf::Pdf;
use crate::progress::step;
use lopdf::Object;

pub fn normalize_degrees(degrees: i32) -> i32 {
    let mut d = degrees % 360;
    if d < 0 {
        d += 360;
    }
    d
}

pub fn rotate(
    pdf: &mut Pdf,
    pages: &[u32],
    degrees: i32,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    if degrees % 90 != 0 {
        return Err(Error::InvalidInput(
            "rotation must be a multiple of 90".to_owned(),
        ));
    }
    let mut state = ThrottleState::new();
    for (i, page_num) in pages.iter().enumerate() {
        step(
            cancel,
            progress,
            &mut state,
            i as u64,
            pages.len() as u64,
            "rotate",
        )?;
        let page_id = pdf.page_id(*page_num)?;
        let current = resolve_rotate(pdf.document(), page_id).map_err(map_err)?;
        let next = normalize_degrees(current + degrees);
        let page = pdf
            .document_mut()
            .get_dictionary_mut(page_id)
            .map_err(map_err)?;
        page.set("Rotate", Object::Integer(next as i64));
    }
    Ok(())
}
