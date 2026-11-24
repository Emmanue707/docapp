use std::path::Path;
use std::sync::{Arc, Mutex};

use docsuite_core::{CancelToken, Error, Result};
use ocrs::TextItem;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams, OcrError};
use rten_imageproc::{BoundingRect, RotatedRect};

use crate::gray::GrayImage;
use crate::models::{self, ensure_models};
use crate::preprocess::crop_gray;
use crate::recognizer::Recognizer;
use crate::types::{BBox, Candidate, Line, Word};

#[derive(Clone)]
pub struct OcrModels {
    pub detection: std::path::PathBuf,
    pub recognition: std::path::PathBuf,
}

impl OcrModels {
    pub fn from_dir(dir: &Path) -> Result<Self> {
        ensure_models(dir)?;
        let (det, rec) = models::model_paths(dir);
        Ok(Self {
            detection: det,
            recognition: rec,
        })
    }
}

fn map_ocr_err(e: OcrError) -> Error {
    Error::Ocr(e.to_string())
}

fn word_confidence(_word: &ocrs::TextWord<'_>) -> f32 {
    1.0
}

fn lines_from_ocrs(
    ocr_lines: &[Option<ocrs::TextLine>],
    word_rects: &[RotatedRect],
    offset_x: f32,
    offset_y: f32,
) -> Vec<Line> {
    let mut out = Vec::new();
    let mut word_iter = word_rects.iter();
    for line_opt in ocr_lines {
        let Some(line) = line_opt else {
            continue;
        };
        let line_rect = line.bounding_rect();
        let mut words = Vec::new();
        for word in line.words() {
            let text: String = word.to_string();
            if text.is_empty() {
                continue;
            }
            let wrect = word.rotated_rect().bounding_rect();
            #[allow(clippy::unnecessary_cast)]
            let bbox = BBox {
                x0: wrect.left() as f32 + offset_x,
                y0: wrect.top() as f32 + offset_y,
                x1: wrect.right() as f32 + offset_x,
                y1: wrect.bottom() as f32 + offset_y,
            };
            words.push(Word {
                bbox,
                candidates: vec![Candidate {
                    text,
                    confidence: word_confidence(&word),
                }],
            });
            let _ = word_iter.next();
        }
        if words.is_empty() {
            continue;
        }
        let lb = BBox {
            x0: line_rect.left() as f32 + offset_x,
            y0: line_rect.top() as f32 + offset_y,
            x1: line_rect.right() as f32 + offset_x,
            y1: line_rect.bottom() as f32 + offset_y,
        };
        out.push(Line { bbox: lb, words });
    }
    out
}

fn gray_to_source(image: &GrayImage) -> Result<ImageSource<'_>> {
    ImageSource::from_bytes(&image.pixels, (image.width, image.height))
        .map_err(|e| Error::Ocr(e.to_string()))
}

fn build_engine(models: &OcrModels) -> Result<OcrEngine> {
    let detection =
        rten::Model::load_file(&models.detection).map_err(|e| Error::Ocr(e.to_string()))?;
    let recognition =
        rten::Model::load_file(&models.recognition).map_err(|e| Error::Ocr(e.to_string()))?;
    OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection),
        recognition_model: Some(recognition),
        ..Default::default()
    })
    .map_err(map_ocr_err)
}

pub struct OcrsRecognizer {
    models: OcrModels,
    engine: Mutex<Option<OcrEngine>>,
}

impl OcrsRecognizer {
    pub fn new(models: OcrModels) -> Self {
        Self {
            models,
            engine: Mutex::new(None),
        }
    }

    pub fn with_dir(dir: &Path) -> Result<Self> {
        Ok(Self::new(OcrModels::from_dir(dir)?))
    }

    fn with_engine<R>(&self, f: impl FnOnce(&OcrEngine) -> Result<R>) -> Result<R> {
        let mut guard = self
            .engine
            .lock()
            .map_err(|_| Error::Ocr("ocr engine lock poisoned".to_owned()))?;
        if guard.is_none() {
            *guard = Some(build_engine(&self.models)?);
        }
        let engine = guard.as_ref().expect("engine");
        f(engine)
    }

    pub fn recognize_gray(
        &self,
        image: &GrayImage,
        region: Option<BBox>,
        cancel: &CancelToken,
    ) -> Result<Vec<Line>> {
        cancel.check()?;
        let (work, ox, oy) = if let Some(r) = region {
            (crop_gray(image, r), r.x0, r.y0)
        } else {
            (image.clone(), 0.0, 0.0)
        };
        let source = gray_to_source(&work)?;
        cancel.check()?;
        self.with_engine(|engine| {
            let input = engine.prepare_input(source).map_err(map_ocr_err)?;
            let words = engine.detect_words(&input).map_err(map_ocr_err)?;
            cancel.check()?;
            let line_groups = engine.find_text_lines(&input, &words);
            let ocr_lines = engine
                .recognize_text(&input, &line_groups)
                .map_err(map_ocr_err)?;
            Ok(lines_from_ocrs(&ocr_lines, &words, ox, oy))
        })
    }
}

impl Recognizer for OcrsRecognizer {
    fn name(&self) -> &str {
        "ocrs"
    }

    fn recognize(
        &self,
        image: &GrayImage,
        region: Option<BBox>,
        cancel: &CancelToken,
    ) -> Result<Vec<Line>> {
        self.recognize_gray(image, region, cancel)
    }
}

pub type SharedRecognizer = Arc<OcrsRecognizer>;

pub fn shared_with_dir(dir: &Path) -> Result<SharedRecognizer> {
    Ok(Arc::new(OcrsRecognizer::with_dir(dir)?))
}

#[cfg(all(test, feature = "ocrs"))]
mod live {
    use std::path::Path;

    use docsuite_core::CancelToken;
    use image::GenericImageView;

    use super::OcrsRecognizer;
    use crate::models::default_dir;
    use crate::preprocess::{MAX_OCR_LONG_SIDE, cap_long_side};

    #[test]
    #[ignore]
    fn ocr_env_sample() {
        let path = match std::env::var("DOCAPP_OCR_SAMPLE") {
            Ok(p) => p,
            Err(_) => return,
        };
        let bytes = std::fs::read(&path).expect("read sample");
        let img = image::load_from_memory(&bytes).expect("decode");
        let rgba = img.to_rgba8();
        let (w, h) = img.dimensions();
        let gray = crate::preprocess::rgba_to_grayscale(w, h, rgba.as_raw()).expect("gray");
        let (scaled, _) = cap_long_side(gray, MAX_OCR_LONG_SIDE);
        let rec = OcrsRecognizer::with_dir(Path::new(&default_dir())).expect("models");
        let lines = rec
            .recognize_gray(&scaled, None, &CancelToken::new())
            .expect("ocr");
        assert!(!lines.is_empty());
    }
}
