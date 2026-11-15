use crate::attribute_base_table;
use crate::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

/// The attribute set of a living entity type and the base of the type of each attribute in it
/// (`attributes.md`, ATR-BASE-001 to ATR-BASE-003).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributeBases {
    set: AttributeMask,
    differing: &'static [TypeBase],
}

impl AttributeBases {
    /// The bases of `entity_type`, or `None` for a type that is not a living entity type.
    #[must_use]
    pub const fn of(entity_type: EntityType) -> Option<Self> {
        attribute_base_table::of(entity_type)
    }

    /// The base of the type of `attribute`, or `None` when the attribute is not in the set.
    #[must_use]
    pub fn base(self, attribute: Attribute) -> Option<f64> {
        if !self.set.contains(attribute) {
            return None;
        }
        let differing = self
            .differing
            .iter()
            .find(|entry| entry.attribute == attribute);
        Some(differing.map_or(attribute.definition().default, |entry| entry.base))
    }

    /// The attributes of the set, in the order of their numeric ids.
    pub fn attributes(self) -> impl Iterator<Item = Attribute> {
        Attribute::ALL
            .iter()
            .copied()
            .filter(move |attribute| self.set.contains(*attribute))
    }

    pub(crate) const fn new(more: &[Attribute], differing: &'static [TypeBase]) -> Self {
        let mut set = AttributeMask::COMMON;
        let mut rest = more;
        while let [attribute, tail @ ..] = rest {
            set = set.with(*attribute);
            rest = tail;
        }
        Self { set, differing }
    }
}

/// A base of a type that differs from the attribute default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TypeBase {
    pub(crate) attribute: Attribute,
    pub(crate) base: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AttributeMask(u64);

impl AttributeMask {
    const COMMON: Self = Self::EMPTY
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

    const EMPTY: Self = Self(0);

    const fn bit(attribute: Attribute) -> u64 {
        match 1u64.checked_shl(attribute.id().number() as u32) {
            Some(bit) => bit,
            None => 0,
        }
    }

    const fn with(self, attribute: Attribute) -> Self {
        Self(self.0 | Self::bit(attribute))
    }

    const fn contains(self, attribute: Attribute) -> bool {
        self.0 & Self::bit(attribute) != 0
    }
}

const _: () = assert!(
    Attribute::ALL.len() <= u64::BITS as usize,
    "each attribute has one bit of the mask"
);
