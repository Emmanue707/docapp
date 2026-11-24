mod gray;
mod models;
mod predictor;
mod preprocess;
mod recognizer;
mod scorer;
mod types;

#[cfg(feature = "ocrs")]
mod ocrs_recognizer;

pub use gray::GrayImage;
pub use models::{
    DETECTION_FILE, DETECTION_SHA256, ModelStatus, RECOGNITION_FILE, RECOGNITION_SHA256,
    default_dir, ensure_models, model_paths, status, verify_file,
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
