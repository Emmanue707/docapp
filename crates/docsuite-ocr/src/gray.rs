#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl GrayImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        if pixels.len() != (width as usize) * (height as usize) {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn get(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.pixels[y as usize * self.width as usize + x as usize]
    }
}
