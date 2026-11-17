#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MotionEffects {
    pub slow_falling: bool,
    pub levitation: Option<u8>,
    pub jump_boost: Option<u8>,
    pub weaving: bool,
    pub blindness: bool,
}
