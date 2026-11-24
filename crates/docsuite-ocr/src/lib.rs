use docsuite_core::{Error, Result};

pub struct Recognizer;

impl Recognizer {
    pub fn recognize_text(_bytes: &[u8]) -> Result<String> {
        Err(Error::Unsupported("ocr not implemented".to_owned()))
    }
}
