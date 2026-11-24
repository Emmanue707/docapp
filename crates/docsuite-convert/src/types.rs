#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub fn from_pdf_rgb(r: f64, g: f64, b: f64) -> Self {
        Self {
            r: (r.clamp(0.0, 1.0) * 255.0).round() as u8,
            g: (g.clamp(0.0, 1.0) * 255.0).round() as u8,
            b: (b.clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }

    pub fn hex(&self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub font_size: f64,
    pub font_key: String,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
    pub color: RgbColor,
    pub char_spacing: f64,
    pub hidden: bool,
    pub confidence: f32,
    pub highlight: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    pub runs: Vec<TextRun>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedImage {
    pub bytes: Vec<u8>,
    pub kind: ImageKind,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub behind: bool,
    pub orient_fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PathSegment {
    MoveTo(PathPoint),
    LineTo(PathPoint),
    CurveTo(PathPoint, PathPoint, PathPoint),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorPath {
    pub segments: Vec<PathSegment>,
    pub fill: Option<RgbColor>,
    pub stroke: Option<RgbColor>,
    pub line_width: f64,
}

#[derive(Clone, Debug)]
pub struct ExtractedPage {
    pub width: f64,
    pub height: f64,
    pub rotate: i32,
    pub source_rotate: i32,
    pub crop_box: [f64; 4],
    pub text: Vec<TextLine>,
    pub images: Vec<PlacedImage>,
    pub vectors: Vec<VectorPath>,
}

impl ExtractedPage {
    pub fn empty_a4() -> Self {
        Self {
            width: 595.0,
            height: 842.0,
            rotate: 0,
            source_rotate: 0,
            crop_box: [0.0, 0.0, 595.0, 842.0],
            text: Vec::new(),
            images: Vec::new(),
            vectors: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FontEmbed {
    pub pdf_name: String,
    pub family: String,
    pub ttf_bytes: Vec<u8>,
    pub bold: bool,
    pub italic: bool,
}

impl FontEmbed {
    pub fn style_tag(&self) -> &'static str {
        match (self.bold, self.italic) {
            (true, true) => "boldItalic",
            (true, false) => "bold",
            (false, true) => "italic",
            (false, false) => "regular",
        }
    }
}
