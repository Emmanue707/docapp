use docsuite_core::{Error, Result};

pub struct Converter;

impl Converter {
    pub fn convert(_input: &str, _output: &str) -> Result<()> {
        Err(Error::Unsupported("convert not implemented".to_owned()))
    }
}
