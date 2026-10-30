/// One axis of a world position or of a rotation: a value, or an offset from the source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCoordinate {
    /// The absolute value, or the offset when `relative` is set.
    pub value: f64,
    /// Whether `value` is an offset from the value of the source (`~`).
    pub relative: bool,
}

impl WorldCoordinate {
    /// The value of the axis against `origin`, the value of the source on that axis.
    #[must_use]
    pub fn resolve(self, origin: f64) -> f64 {
        if self.relative {
            self.value + origin
        } else {
            self.value
        }
    }
}
