#![allow(clippy::too_many_arguments)]

use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};
use lopdf::Object;

use crate::lopdf_map::map_err;
use crate::page_attr::mediabox_array;
use crate::pdf::Pdf;
use crate::progress::step;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfRect {
    pub llx: f64,
    pub lly: f64,
    pub urx: f64,
    pub ury: f64,
}

impl PdfRect {
    pub fn contains(&self, other: &Self) -> bool {
        self.llx <= other.llx
            && self.lly <= other.lly
            && self.urx >= other.urx
            && self.ury >= other.ury
    }
}

fn rect_from_mediabox(boxes: [f64; 4]) -> PdfRect {
    PdfRect {
        llx: boxes[0],
        lly: boxes[1],
        urx: boxes[2],
        ury: boxes[3],
    }
}

pub fn crop(
    pdf: &mut Pdf,
    pages: &[u32],
    crop: PdfRect,
    adjust_media: bool,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    if crop.urx <= crop.llx || crop.ury <= crop.lly {
        return Err(Error::InvalidInput("invalid crop rectangle".to_owned()));
    }
    let mut state = ThrottleState::new();
    for (i, page_num) in pages.iter().enumerate() {
        step(
            cancel,
            progress,
            &mut state,
            i as u64,
            pages.len() as u64,
            "crop",
        )?;
        let page_id = pdf.page_id(*page_num)?;
        let media = mediabox_array(pdf.document(), page_id).map_err(map_err)?;
        let media_rect = rect_from_mediabox(media);
        if !media_rect.contains(&crop) {
            return Err(Error::InvalidInput(format!(
                "crop box must lie inside MediaBox on page {page_num}"
            )));
        }
        let crop_object = Object::Array(vec![
            Object::Real(crop.llx as f32),
            Object::Real(crop.lly as f32),
            Object::Real(crop.urx as f32),
            Object::Real(crop.ury as f32),
        ]);
        let page = pdf
            .document_mut()
            .get_dictionary_mut(page_id)
            .map_err(map_err)?;
        page.set("CropBox", crop_object.clone());
        if adjust_media {
            page.set("MediaBox", crop_object);
        }
    }
    Ok(())
}
