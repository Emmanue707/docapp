use docsuite_core::{CancelToken, Result};

use crate::gray::GrayImage;
use crate::types::{BBox, Line};

pub trait Recognizer: Send + Sync {
    fn name(&self) -> &str;
    fn recognize(
        &self,
        image: &GrayImage,
        region: Option<BBox>,
        cancel: &CancelToken,
    ) -> Result<Vec<Line>>;
}
