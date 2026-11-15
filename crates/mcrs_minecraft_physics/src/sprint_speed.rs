/// The value of `movement_speed` that the kernel reads while the sprint state is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprintSpeed(pub f64);

impl Default for SprintSpeed {
    /// The value for the base of a player with no modifier but the sprint modifier.
    fn default() -> Self {
        Self(f64::from(0.1_f32) * (1.0 + f64::from(0.3_f32)))
    }
}
