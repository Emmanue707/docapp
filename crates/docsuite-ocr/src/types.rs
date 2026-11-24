#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl BBox {
    pub fn width(&self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }

    pub fn height(&self) -> f32 {
        (self.y1 - self.y0).max(0.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub text: String,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub bbox: BBox,
    pub candidates: Vec<Candidate>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub bbox: BBox,
    pub words: Vec<Word>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    Date,
    Currency,
    CodeId,
    Email,
    Number,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedGuess {
    pub field_type: FieldType,
    pub normalized: Option<String>,
    pub score: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoredCandidate {
    pub text: String,
    pub confidence: f32,
    pub combined_score: f32,
    pub guess: Option<TypedGuess>,
}
