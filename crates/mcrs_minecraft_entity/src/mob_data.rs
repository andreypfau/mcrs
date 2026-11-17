#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MobData {
    pub head_turn_speed: f32,
    pub pitch_limit: f32,
    pub head_yaw_limit: f32,
    pub ageable: bool,
}

impl MobData {
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
