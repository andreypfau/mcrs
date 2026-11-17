use crate::ValueRange;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributeDefinition {
    pub default: f64,
    pub range: ValueRange,
    pub syncable: bool,
}

const fn definition(default: f64, min: f64, max: f64, syncable: bool) -> AttributeDefinition {
    AttributeDefinition {
        default,
        range: ValueRange { min, max },
        syncable,
    }
}

impl crate::attribute_keys::Attribute {
    pub const fn definition(self) -> AttributeDefinition {
        match self {
            Self::AirDragModifier => definition(1.0, 0.0, 2048.0, true),
            Self::Armor => definition(0.0, 0.0, 30.0, true),
            Self::ArmorToughness => definition(0.0, 0.0, 20.0, true),
            Self::AttackDamage => definition(2.0, 0.0, 2048.0, false),
            Self::AttackKnockback => definition(0.0, 0.0, 5.0, false),
            Self::AttackSpeed => definition(4.0, 0.0, 1024.0, true),
            Self::BelowNameDistance => definition(10.0, 0.0, 512.0, true),
            Self::BlockBreakSpeed => definition(1.0, 0.0, 1024.0, true),
            Self::BlockInteractionRange => definition(4.5, 0.0, 64.0, true),
            Self::Bounciness => definition(0.0, 0.0, 1.0, true),
            Self::BurningTime => definition(1.0, 0.0, 1024.0, true),
            Self::CameraDistance => definition(4.0, 0.0, 32.0, true),
            Self::ExplosionKnockbackResistance => definition(0.0, 0.0, 1.0, true),
            Self::EntityInteractionRange => definition(3.0, 0.0, 64.0, true),
            Self::FallDamageMultiplier => definition(1.0, 0.0, 100.0, true),
            Self::FlyingSpeed => definition(0.4, 0.0, 1024.0, true),
            Self::FollowRange => definition(32.0, 0.0, 2048.0, false),
            Self::FrictionModifier => definition(1.0, 0.0, 2048.0, true),
            Self::Gravity => definition(0.08, -1.0, 1.0, true),
            Self::JumpStrength => definition(0.42_f32 as f64, 0.0, 32.0, true),
            Self::KnockbackResistance => definition(0.0, -2.0, 1.0, false),
            Self::Luck => definition(0.0, -1024.0, 1024.0, true),
            Self::MaxAbsorption => definition(0.0, 0.0, 2048.0, true),
            Self::MaxHealth => definition(20.0, 1.0, 1024.0, true),
            Self::MiningEfficiency => definition(0.0, 0.0, 1024.0, true),
            Self::MovementEfficiency => definition(0.0, 0.0, 1.0, true),
            Self::MovementSpeed => definition(0.7, 0.0, 1024.0, true),
            Self::NameTagDistance => definition(64.0, 0.0, 512.0, true),
            Self::OxygenBonus => definition(0.0, 0.0, 1024.0, true),
            Self::SafeFallDistance => definition(3.0, -1024.0, 1024.0, true),
            Self::Scale => definition(1.0, 0.0625, 16.0, true),
            Self::SneakingSpeed => definition(0.3, 0.0, 1.0, true),
            Self::SpawnReinforcements => definition(0.0, 0.0, 1.0, false),
            Self::StepHeight => definition(0.6, 0.0, 10.0, true),
            Self::SubmergedMiningSpeed => definition(0.2, 0.0, 20.0, true),
            Self::SweepingDamageRatio => definition(0.0, 0.0, 1.0, true),
            Self::TemptRange => definition(10.0, 0.0, 2048.0, false),
            Self::WaterMovementEfficiency => definition(0.0, 0.0, 1.0, true),
            Self::WaypointTransmitRange => definition(0.0, 0.0, 60000000.0, false),
            Self::WaypointReceiveRange => definition(0.0, 0.0, 60000000.0, false),
        }
    }
}
