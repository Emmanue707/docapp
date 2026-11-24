mod gray;
mod predictor;
mod preprocess;
mod recognizer;
mod scorer;
mod types;

pub use gray::GrayImage;
pub use predictor::{apply_confusions_for_scoring, predict_type};
pub use preprocess::{
    binarize_otsu, crop_gray, estimate_deskew_degrees, horizontal_projection_lines, otsu_threshold,
    rgba_to_grayscale,
};
pub use recognizer::Recognizer;
pub use scorer::{rerank_top, score_candidates};
pub use types::{BBox, Candidate, FieldType, Line, ScoredCandidate, TypedGuess, Word};
