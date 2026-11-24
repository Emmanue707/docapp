use crate::cms_builder::PdfSubFilter;
use crate::crop::PdfRect;

#[derive(Clone, Debug)]
pub struct SignatureStamp {
    pub name: String,
    pub reason: String,
    pub date: String,
    pub image: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct DigitalSignOptions {
    pub field_name: String,
    pub sub_filter: PdfSubFilter,
    pub reason: Option<String>,
    pub location: Option<String>,
    pub contact: Option<String>,
    pub visible: Option<(u32, PdfRect, SignatureStamp)>,
}
