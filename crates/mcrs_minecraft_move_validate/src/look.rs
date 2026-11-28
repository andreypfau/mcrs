#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Look {
    pub yaw: f32,
    pub pitch: f32,
}

impl Look {
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.yaw.is_finite() && self.pitch.is_finite()
    }
}
