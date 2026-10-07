#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributeDefinition {
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub syncable: bool,
}

impl AttributeDefinition {
    pub const fn new(default: f64, min: f64, max: f64, syncable: bool) -> Self {
        Self {
            default,
            min,
            max,
            syncable,
        }
    }
}

impl crate::keys::Attribute {
    pub const fn definition(self) -> AttributeDefinition {
        match self {
            Self::AirDragModifier => AttributeDefinition::new(1.0, 0.0, 2048.0, true),
            Self::Armor => AttributeDefinition::new(0.0, 0.0, 30.0, true),
            Self::ArmorToughness => AttributeDefinition::new(0.0, 0.0, 20.0, true),
            Self::AttackDamage => AttributeDefinition::new(2.0, 0.0, 2048.0, false),
            Self::AttackKnockback => AttributeDefinition::new(0.0, 0.0, 5.0, false),
            Self::AttackSpeed => AttributeDefinition::new(4.0, 0.0, 1024.0, true),
            Self::BelowNameDistance => AttributeDefinition::new(10.0, 0.0, 512.0, true),
            Self::BlockBreakSpeed => AttributeDefinition::new(1.0, 0.0, 1024.0, true),
            Self::BlockInteractionRange => AttributeDefinition::new(4.5, 0.0, 64.0, true),
            Self::Bounciness => AttributeDefinition::new(0.0, 0.0, 1.0, true),
            Self::BurningTime => AttributeDefinition::new(1.0, 0.0, 1024.0, true),
            Self::CameraDistance => AttributeDefinition::new(4.0, 0.0, 32.0, true),
            Self::ExplosionKnockbackResistance => AttributeDefinition::new(0.0, 0.0, 1.0, true),
            Self::EntityInteractionRange => AttributeDefinition::new(3.0, 0.0, 64.0, true),
            Self::FallDamageMultiplier => AttributeDefinition::new(1.0, 0.0, 100.0, true),
            Self::FlyingSpeed => AttributeDefinition::new(0.4, 0.0, 1024.0, true),
            Self::FollowRange => AttributeDefinition::new(32.0, 0.0, 2048.0, false),
            Self::FrictionModifier => AttributeDefinition::new(1.0, 0.0, 2048.0, true),
            Self::Gravity => AttributeDefinition::new(0.08, -1.0, 1.0, true),
            Self::JumpStrength => AttributeDefinition::new(0.42_f32 as f64, 0.0, 32.0, true),
            Self::KnockbackResistance => AttributeDefinition::new(0.0, -2.0, 1.0, false),
            Self::Luck => AttributeDefinition::new(0.0, -1024.0, 1024.0, true),
            Self::MaxAbsorption => AttributeDefinition::new(0.0, 0.0, 2048.0, true),
            Self::MaxHealth => AttributeDefinition::new(20.0, 1.0, 1024.0, true),
            Self::MiningEfficiency => AttributeDefinition::new(0.0, 0.0, 1024.0, true),
            Self::MovementEfficiency => AttributeDefinition::new(0.0, 0.0, 1.0, true),
            Self::MovementSpeed => AttributeDefinition::new(0.7, 0.0, 1024.0, true),
            Self::NameTagDistance => AttributeDefinition::new(64.0, 0.0, 512.0, true),
            Self::OxygenBonus => AttributeDefinition::new(0.0, 0.0, 1024.0, true),
            Self::SafeFallDistance => AttributeDefinition::new(3.0, -1024.0, 1024.0, true),
            Self::Scale => AttributeDefinition::new(1.0, 0.0625, 16.0, true),
            Self::SneakingSpeed => AttributeDefinition::new(0.3, 0.0, 1.0, true),
            Self::SpawnReinforcements => AttributeDefinition::new(0.0, 0.0, 1.0, false),
            Self::StepHeight => AttributeDefinition::new(0.6, 0.0, 10.0, true),
            Self::SubmergedMiningSpeed => AttributeDefinition::new(0.2, 0.0, 20.0, true),
            Self::SweepingDamageRatio => AttributeDefinition::new(0.0, 0.0, 1.0, true),
            Self::TemptRange => AttributeDefinition::new(10.0, 0.0, 2048.0, false),
            Self::WaterMovementEfficiency => AttributeDefinition::new(0.0, 0.0, 1.0, true),
            Self::WaypointTransmitRange => AttributeDefinition::new(0.0, 0.0, 60000000.0, false),
            Self::WaypointReceiveRange => AttributeDefinition::new(0.0, 0.0, 60000000.0, false),
        }
    }
}
