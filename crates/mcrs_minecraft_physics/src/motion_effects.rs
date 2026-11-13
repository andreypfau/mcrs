/// The effects that change the motion of a living entity; a level is the amplifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MotionEffects {
    pub speed: Option<u8>,
    pub slowness: Option<u8>,
    pub slow_falling: bool,
    pub levitation: Option<u8>,
    pub jump_boost: Option<u8>,
    pub weaving: bool,
    pub blindness: bool,
}

impl MotionEffects {
    /// The movement speed after the `speed` and `slowness` effects, clamped as the motion step
    /// reads it.
    #[must_use]
    pub fn speed_with_effects(&self, base: f64) -> f64 {
        let speed = self
            .speed
            .map_or(1.0, |level| 1.0 + 0.2 * f64::from(level.saturating_add(1)));
        let slowness = self.slowness.map_or(1.0, |level| {
            (1.0 - 0.15 * f64::from(level.saturating_add(1))).max(0.0)
        });
        (base * speed * slowness).clamp(0.0, 1024.0)
    }
}
