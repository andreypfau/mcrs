use crate::DamageSource;
use crate::damage_rules::DamageTypeFlags;

/// A hit on a living entity: the amount, the damage type id, and the full damage source (DMG-SRC-001).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Hit {
    pub amount: f32,
    pub damage_type: u32,
    pub source: DamageSource,
    pub knockback: f32,
    pub flags: DamageTypeFlags,
}

impl Hit {
    #[must_use]
    pub const fn new(amount: f32, damage_type: u32) -> Self {
        Self {
            amount,
            damage_type,
            source: DamageSource::new(damage_type),
            knockback: 0.4,
            flags: DamageTypeFlags::builtin_for(damage_type),
        }
    }

    #[must_use]
    pub const fn from_source(amount: f32, source: DamageSource) -> Self {
        Self {
            amount,
            damage_type: source.damage_type,
            source,
            knockback: 0.4,
            flags: DamageTypeFlags::builtin_for(source.damage_type),
        }
    }

    #[must_use]
    pub const fn with_flags(mut self, flags: DamageTypeFlags) -> Self {
        self.flags = flags;
        self
    }
}
