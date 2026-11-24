use std::sync::Arc;

use docsuite_core::{CancelToken, Result};
use docsuite_ocr::{BBox, GrayImage, Line, Recognizer};

use crate::ocr_util::{MAX_OCR_LONG_SIDE, cap_long_side, decode_image_gray, lines_to_text_runs};
use crate::types::{PlacedImage, TextRun};

pub struct RecognizerPageOcr {
    pub recognizer: Arc<dyn Recognizer>,
    pub highlight_low_confidence: bool,
}

impl RecognizerPageOcr {
    pub fn ocr_placed(
        &self,
        img: &PlacedImage,
        region: Option<BBox>,
        cancel: &CancelToken,
    ) -> Result<(Vec<Line>, u32, u32)> {
        let gray = decode_image_gray(&img.bytes)?;
        let (scaled, _s) = cap_long_side(gray, MAX_OCR_LONG_SIDE);
        let region = region.map(|r| BBox {
            x0: r.x0,
            y0: r.y0,
            x1: r.x1,
            y1: r.y1,
        });
        let lines = self.recognizer.recognize(&scaled, region, cancel)?;
        Ok((lines, scaled.width, scaled.height))
    }

    pub fn runs_for_image(
        &self,
        img: &PlacedImage,
        region: Option<BBox>,
        cancel: &CancelToken,
    ) -> Result<Vec<TextRun>> {
        let (lines, ow, oh) = self.ocr_placed(img, region, cancel)?;
        Ok(lines_to_text_runs(
            &lines,
            img,
            ow,
            oh,
            self.highlight_low_confidence,
        ))
    }
}

impl crate::PageOcr for RecognizerPageOcr {
    fn ocr_page(
        &self,
        image: &[u8],
        width: u32,
        height: u32,
        cancel: &CancelToken,
    ) -> Result<Vec<TextRun>> {
        let img = PlacedImage {
            bytes: image.to_vec(),
            kind: crate::types::ImageKind::Png,
            x: 0.0,
            y: 0.0,
            width: width as f64,
            height: height as f64,
            behind: false,
        };
        self.runs_for_image(&img, None, cancel)
    }

    fn ocr_placed(&self, img: &PlacedImage, cancel: &CancelToken) -> Result<Vec<TextRun>> {
        self.runs_for_image(img, None, cancel)
    }
}
