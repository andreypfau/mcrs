#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCoordinate {
    pub value: f64,
    pub relative: bool,
}

impl WorldCoordinate {
    #[must_use]
    pub fn resolve(self, origin: f64) -> f64 {
        if self.relative {
            self.value + origin
        } else {
            self.value
        }
    }
}
