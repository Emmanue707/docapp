#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix(pub [f64; 6]);

impl Matrix {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn concat(self, other: Self) -> Self {
        let a = self.0;
        let b = other.0;
        Self([
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ])
    }

    pub fn transform(&self, x: f64, y: f64) -> (f64, f64) {
        let m = self.0;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }

    pub fn scale_x(&self) -> f64 {
        (self.0[0].powi(2) + self.0[1].powi(2)).sqrt()
    }

    pub fn translate(tx: f64, ty: f64) -> Self {
        Self([1.0, 0.0, 0.0, 1.0, tx, ty])
    }

    pub fn from_pdf_operands(v: &[f64]) -> Self {
        if v.len() != 6 {
            return Self::IDENTITY;
        }
        Self([v[0], v[1], v[2], v[3], v[4], v[5]])
    }
}
