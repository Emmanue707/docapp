use docsuite_core::{Error, Result};

pub struct PdfEngine;

impl PdfEngine {
    pub fn load(_path: &str) -> Result<Self> {
        Err(Error::Unsupported("pdf not implemented".to_owned()))
    }
}
