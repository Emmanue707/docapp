use docsuite_core::{Error, Result};

pub struct DocumentView;

impl DocumentView {
    pub fn open(_path: &str) -> Result<Self> {
        Err(Error::Unsupported("viewer not implemented".to_owned()))
    }
}
