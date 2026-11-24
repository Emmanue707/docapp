mod gray;
mod models;
mod predictor;
mod preprocess;
mod recognizer;
mod scorer;
mod types;

#[cfg(feature = "htr")]
mod ctc;
#[cfg(feature = "htr")]
mod htr_recognizer;
#[cfg(feature = "ocrs")]
mod ocrs_recognizer;

pub use gray::GrayImage;
pub use models::{
    DETECTION_FILE, DETECTION_SHA256, HANDWRITING_FILE, HANDWRITING_SHA256, ModelStatus,
    RECOGNITION_FILE, RECOGNITION_SHA256, default_dir, ensure_handwriting, ensure_models,
    handwriting_path, model_paths, status, verify_file,
};
pub use predictor::{apply_confusions_for_scoring, predict_type};
pub use preprocess::{
    MAX_OCR_LONG_SIDE, binarize_otsu, cap_long_side, crop_gray, estimate_deskew_degrees,
    horizontal_projection_lines, otsu_threshold, rgba_to_grayscale,
};
pub use recognizer::Recognizer;
pub use scorer::{rerank_top, score_candidates};
pub use types::{BBox, Candidate, FieldType, Line, ScoredCandidate, TypedGuess, Word};

#[cfg(feature = "ocrs")]
pub use ocrs_recognizer::{OcrModels, OcrsRecognizer, SharedRecognizer, shared_with_dir};

#[cfg(feature = "htr")]
pub use htr_recognizer::HtrRecognizer;

pub fn open_recognizer(
    dir: &std::path::Path,
    handwriting: bool,
) -> docsuite_core::Result<std::sync::Arc<dyn Recognizer>> {
    use std::sync::Arc;
    #[cfg(all(feature = "htr", feature = "ocrs"))]
    {
        if handwriting {
            Ok(Arc::new(HtrRecognizer::with_dir(dir)?))
        } else {
            Ok(Arc::new(OcrsRecognizer::with_dir(dir)?))
        }
    }
    #[cfg(all(feature = "ocrs", not(feature = "htr")))]
    {
        let _ = handwriting;
        Ok(Arc::new(OcrsRecognizer::with_dir(dir)?))
    }
    #[cfg(not(feature = "ocrs"))]
    {
        let _ = (dir, handwriting);
        Err(docsuite_core::Error::Unsupported(
            "ocr not enabled".to_owned(),
        ))
    }
}
