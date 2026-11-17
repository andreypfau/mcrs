use crate::attribute_keys::Attribute;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AttributeMask(u64);

impl AttributeMask {
    pub(crate) const COMMON: Self = Self::EMPTY
        .with(Attribute::AirDragModifier)
        .with(Attribute::Armor)
        .with(Attribute::ArmorToughness)
        .with(Attribute::AttackKnockback)
        .with(Attribute::BelowNameDistance)
        .with(Attribute::Bounciness)
        .with(Attribute::BurningTime)
        .with(Attribute::CameraDistance)
        .with(Attribute::EntityInteractionRange)
        .with(Attribute::ExplosionKnockbackResistance)
        .with(Attribute::FallDamageMultiplier)
        .with(Attribute::FrictionModifier)
        .with(Attribute::Gravity)
        .with(Attribute::JumpStrength)
        .with(Attribute::KnockbackResistance)
        .with(Attribute::MaxAbsorption)
        .with(Attribute::MaxHealth)
        .with(Attribute::MovementEfficiency)
        .with(Attribute::MovementSpeed)
        .with(Attribute::NameTagDistance)
        .with(Attribute::OxygenBonus)
        .with(Attribute::SafeFallDistance)
        .with(Attribute::Scale)
        .with(Attribute::StepHeight)
        .with(Attribute::WaterMovementEfficiency)
        .with(Attribute::WaypointTransmitRange);

    pub const EMPTY: Self = Self(0);

    const fn bit(attribute: Attribute) -> u64 {
        match 1u64.checked_shl(attribute.id().number() as u32) {
            Some(bit) => bit,
            None => 0,
        }
    }

    #[must_use]
    pub(crate) const fn with(self, attribute: Attribute) -> Self {
        Self(self.0 | Self::bit(attribute))
    }

    #[must_use]
    pub const fn contains(self, attribute: Attribute) -> bool {
        self.0 & Self::bit(attribute) != 0
    }

    pub fn iter(self) -> impl Iterator<Item = Attribute> {
        Attribute::ALL
            .iter()
            .copied()
            .filter(move |attribute| self.contains(*attribute))
    }
}

const _: () = assert!(
    Attribute::ALL.len() <= u64::BITS as usize,
    "each attribute has one bit of the mask"
);
