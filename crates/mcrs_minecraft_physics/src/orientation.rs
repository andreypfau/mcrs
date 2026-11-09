/// The rotation of an entity in degrees. The yaws are raw: never wrapped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orientation {
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub body_yaw: f32,
}

impl Orientation {
    /// An entity that looks along `yaw` and `pitch`, with its head and body turned to `yaw`.
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
