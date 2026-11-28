#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orientation {
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub body_yaw: f32,
}

impl Orientation {
    #[must_use]
    pub const fn facing(yaw: f32, pitch: f32) -> Self {
        Self {
            yaw,
            pitch,
            head_yaw: yaw,
            body_yaw: yaw,
        }
    }
}
