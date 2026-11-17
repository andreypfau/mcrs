#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64,
}

impl ValueRange {
    #[must_use]
    pub fn clamp(self, value: f64) -> f64 {
        if value.is_nan() || value < self.min {
            self.min
        } else if value > self.max {
            self.max
        } else if value.to_bits() == (-0.0f64).to_bits() && self.min.to_bits() == 0.0f64.to_bits() {
            0.0
        } else {
            value
        }
    }
}
