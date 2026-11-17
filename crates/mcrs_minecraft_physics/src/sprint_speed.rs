#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprintSpeed(pub f64);

impl Default for SprintSpeed {
    fn default() -> Self {
        Self(f64::from(0.1_f32) * (1.0 + f64::from(0.3_f32)))
    }
}
