use crate::MobEffectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EffectInstance {
    pub effect: MobEffectId,
    pub amplifier: u8,
    pub duration: i32,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
}

impl EffectInstance {
    #[must_use]
    pub const fn new(effect: MobEffectId) -> Self {
        Self {
            effect,
            amplifier: 0,
            duration: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
        }
    }
}
