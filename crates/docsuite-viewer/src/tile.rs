#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

impl Tile {
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub doc_id: u64,
    pub page: usize,
    pub zoom_bucket: u32,
    pub tx: i32,
    pub ty: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRectPx {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub const SQRT2: f32 = std::f32::consts::SQRT_2;

pub fn zoom_bucket_for_scale(scale: f32) -> u32 {
    if scale <= 0.0 {
        return 0;
    }
    let log = scale.ln() / SQRT2.ln();
    log.round().max(0.0) as u32
}

pub fn scale_for_zoom_bucket(bucket: u32) -> f32 {
    SQRT2.powi(bucket as i32).max(1e-6)
}
