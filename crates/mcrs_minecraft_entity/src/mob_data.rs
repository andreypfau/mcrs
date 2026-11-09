/// The look and body values of a mob type (`mob-goals.md`, GOAL-ANI-006).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MobData {
    /// The largest change of the head yaw in one tick toward a look target that names no
    /// speed, in degrees.
    pub head_turn_speed: f32,
    /// The largest pitch, up or down, of a look target that names no limit.
    pub pitch_limit: f32,
    /// The largest difference between the head yaw and the body yaw, in degrees.
    pub head_yaw_limit: f32,
    /// The entity data chain of the type holds the ageable mob part: the baby and age-locked
    /// entries.
    pub ageable: bool,
}

impl MobData {
    /// The defaults of a mob type.
    pub const DEFAULT: Self = Self {
        head_turn_speed: 10.0,
        pitch_limit: 40.0,
        head_yaw_limit: 75.0,
        ageable: false,
    };
}

impl Default for MobData {
    fn default() -> Self {
        Self::DEFAULT
    }
}
