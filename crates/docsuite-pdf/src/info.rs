use docsuite_core::Result;

use crate::lopdf_map::map_err;
use crate::page_attr::mediabox_array;
use crate::pdf::Pdf;

#[derive(Clone, Debug, PartialEq)]
pub struct PageSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PdfInfo {
    pub pages: u32,
    pub page_sizes: Vec<PageSize>,
    pub encrypted: bool,
    pub version: String,
}

pub fn pdf_info(pdf: &Pdf) -> Result<PdfInfo> {
    let doc = pdf.document();
    let pages = pdf.page_count();
    let mut page_sizes = Vec::with_capacity(pages as usize);
    for page in 1..=pages {
        let page_id = pdf.page_id(page)?;
        let media = mediabox_array(doc, page_id).map_err(map_err)?;
        page_sizes.push(PageSize {
            width: media[2] - media[0],
            height: media[3] - media[1],
        });
    }
    Ok(PdfInfo {
        pages,
        page_sizes,
        encrypted: doc.is_encrypted(),
        version: doc.version.clone(),
    })
}
