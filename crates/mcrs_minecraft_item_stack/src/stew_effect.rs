use crate::MobEffectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StewEffect {
    pub effect: MobEffectId,
    pub duration: i32,
}

impl StewEffect {
    pub const DEFAULT_DURATION: i32 = 160;
}
