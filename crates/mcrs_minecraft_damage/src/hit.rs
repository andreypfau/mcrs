use crate::DamageSource;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Hit {
    pub amount: f32,
    pub source: DamageSource,
}

impl Hit {
    #[must_use]
    pub const fn new(amount: f32, source: DamageSource) -> Self {
        Self { amount, source }
    }
}
