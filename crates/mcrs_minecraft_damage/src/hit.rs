use crate::DamageSource;

/// A hit on a living entity: the amount, the damage type id, and the full damage source (DMG-SRC-001).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Hit {
    pub amount: f32,
    pub damage_type: u32,
    pub source: DamageSource,
}

impl Hit {
    #[must_use]
    pub const fn new(amount: f32, damage_type: u32) -> Self {
        Self {
            amount,
            damage_type,
            source: DamageSource::new(damage_type),
        }
    }

    #[must_use]
    pub const fn from_source(amount: f32, source: DamageSource) -> Self {
        Self {
            amount,
            damage_type: source.damage_type,
            source,
        }
    }
}
