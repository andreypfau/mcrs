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

pub static AIR_DRAG_MODIFIER: AttributeDefinition =
    AttributeDefinition::new(1.0, 0.0, 2048.0, true);
pub static ARMOR: AttributeDefinition = AttributeDefinition::new(0.0, 0.0, 30.0, true);
pub static ARMOR_TOUGHNESS: AttributeDefinition = AttributeDefinition::new(0.0, 0.0, 20.0, true);
pub static ATTACK_DAMAGE: AttributeDefinition = AttributeDefinition::new(2.0, 0.0, 2048.0, false);
pub static ATTACK_KNOCKBACK: AttributeDefinition = AttributeDefinition::new(0.0, 0.0, 5.0, false);
pub static ATTACK_SPEED: AttributeDefinition = AttributeDefinition::new(4.0, 0.0, 1024.0, true);
pub static BELOW_NAME_DISTANCE: AttributeDefinition =
    AttributeDefinition::new(10.0, 0.0, 512.0, true);
pub static BLOCK_BREAK_SPEED: AttributeDefinition =
    AttributeDefinition::new(1.0, 0.0, 1024.0, true);
pub static BLOCK_INTERACTION_RANGE: AttributeDefinition =
    AttributeDefinition::new(4.5, 0.0, 64.0, true);
pub static BOUNCINESS: AttributeDefinition = AttributeDefinition::new(0.0, 0.0, 1.0, true);
pub static BURNING_TIME: AttributeDefinition = AttributeDefinition::new(1.0, 0.0, 1024.0, true);
pub static CAMERA_DISTANCE: AttributeDefinition = AttributeDefinition::new(4.0, 0.0, 32.0, true);
pub static EXPLOSION_KNOCKBACK_RESISTANCE: AttributeDefinition =
    AttributeDefinition::new(0.0, 0.0, 1.0, true);
pub static ENTITY_INTERACTION_RANGE: AttributeDefinition =
    AttributeDefinition::new(3.0, 0.0, 64.0, true);
pub static FALL_DAMAGE_MULTIPLIER: AttributeDefinition =
    AttributeDefinition::new(1.0, 0.0, 100.0, true);
pub static FLYING_SPEED: AttributeDefinition = AttributeDefinition::new(0.4, 0.0, 1024.0, true);
pub static FOLLOW_RANGE: AttributeDefinition = AttributeDefinition::new(32.0, 0.0, 2048.0, false);
pub static FRICTION_MODIFIER: AttributeDefinition =
    AttributeDefinition::new(1.0, 0.0, 2048.0, true);
pub static GRAVITY: AttributeDefinition = AttributeDefinition::new(0.08, -1.0, 1.0, true);
pub static JUMP_STRENGTH: AttributeDefinition =
    AttributeDefinition::new(0.42_f32 as f64, 0.0, 32.0, true);
pub static KNOCKBACK_RESISTANCE: AttributeDefinition =
    AttributeDefinition::new(0.0, -2.0, 1.0, false);
pub static LUCK: AttributeDefinition = AttributeDefinition::new(0.0, -1024.0, 1024.0, true);
pub static MAX_ABSORPTION: AttributeDefinition = AttributeDefinition::new(0.0, 0.0, 2048.0, true);
pub static MAX_HEALTH: AttributeDefinition = AttributeDefinition::new(20.0, 1.0, 1024.0, true);
pub static MINING_EFFICIENCY: AttributeDefinition =
    AttributeDefinition::new(0.0, 0.0, 1024.0, true);
pub static MOVEMENT_EFFICIENCY: AttributeDefinition = AttributeDefinition::new(0.0, 0.0, 1.0, true);
pub static MOVEMENT_SPEED: AttributeDefinition = AttributeDefinition::new(0.7, 0.0, 1024.0, true);
pub static NAME_TAG_DISTANCE: AttributeDefinition =
    AttributeDefinition::new(64.0, 0.0, 512.0, true);
pub static OXYGEN_BONUS: AttributeDefinition = AttributeDefinition::new(0.0, 0.0, 1024.0, true);
pub static SAFE_FALL_DISTANCE: AttributeDefinition =
    AttributeDefinition::new(3.0, -1024.0, 1024.0, true);
pub static SCALE: AttributeDefinition = AttributeDefinition::new(1.0, 0.0625, 16.0, true);
pub static SNEAKING_SPEED: AttributeDefinition = AttributeDefinition::new(0.3, 0.0, 1.0, true);
pub static SPAWN_REINFORCEMENTS: AttributeDefinition =
    AttributeDefinition::new(0.0, 0.0, 1.0, false);
pub static STEP_HEIGHT: AttributeDefinition = AttributeDefinition::new(0.6, 0.0, 10.0, true);
pub static SUBMERGED_MINING_SPEED: AttributeDefinition =
    AttributeDefinition::new(0.2, 0.0, 20.0, true);
pub static SWEEPING_DAMAGE_RATIO: AttributeDefinition =
    AttributeDefinition::new(0.0, 0.0, 1.0, true);
pub static TEMPT_RANGE: AttributeDefinition = AttributeDefinition::new(10.0, 0.0, 2048.0, false);
pub static WATER_MOVEMENT_EFFICIENCY: AttributeDefinition =
    AttributeDefinition::new(0.0, 0.0, 1.0, true);
pub static WAYPOINT_TRANSMIT_RANGE: AttributeDefinition =
    AttributeDefinition::new(0.0, 0.0, 60000000.0, false);
pub static WAYPOINT_RECEIVE_RANGE: AttributeDefinition =
    AttributeDefinition::new(0.0, 0.0, 60000000.0, false);

impl crate::keys::Attribute {
    pub fn definition(self) -> &'static AttributeDefinition {
        match self {
            Self::AirDragModifier => &AIR_DRAG_MODIFIER,
            Self::Armor => &ARMOR,
            Self::ArmorToughness => &ARMOR_TOUGHNESS,
            Self::AttackDamage => &ATTACK_DAMAGE,
            Self::AttackKnockback => &ATTACK_KNOCKBACK,
            Self::AttackSpeed => &ATTACK_SPEED,
            Self::BelowNameDistance => &BELOW_NAME_DISTANCE,
            Self::BlockBreakSpeed => &BLOCK_BREAK_SPEED,
            Self::BlockInteractionRange => &BLOCK_INTERACTION_RANGE,
            Self::Bounciness => &BOUNCINESS,
            Self::BurningTime => &BURNING_TIME,
            Self::CameraDistance => &CAMERA_DISTANCE,
            Self::ExplosionKnockbackResistance => &EXPLOSION_KNOCKBACK_RESISTANCE,
            Self::EntityInteractionRange => &ENTITY_INTERACTION_RANGE,
            Self::FallDamageMultiplier => &FALL_DAMAGE_MULTIPLIER,
            Self::FlyingSpeed => &FLYING_SPEED,
            Self::FollowRange => &FOLLOW_RANGE,
            Self::FrictionModifier => &FRICTION_MODIFIER,
            Self::Gravity => &GRAVITY,
            Self::JumpStrength => &JUMP_STRENGTH,
            Self::KnockbackResistance => &KNOCKBACK_RESISTANCE,
            Self::Luck => &LUCK,
            Self::MaxAbsorption => &MAX_ABSORPTION,
            Self::MaxHealth => &MAX_HEALTH,
            Self::MiningEfficiency => &MINING_EFFICIENCY,
            Self::MovementEfficiency => &MOVEMENT_EFFICIENCY,
            Self::MovementSpeed => &MOVEMENT_SPEED,
            Self::NameTagDistance => &NAME_TAG_DISTANCE,
            Self::OxygenBonus => &OXYGEN_BONUS,
            Self::SafeFallDistance => &SAFE_FALL_DISTANCE,
            Self::Scale => &SCALE,
            Self::SneakingSpeed => &SNEAKING_SPEED,
            Self::SpawnReinforcements => &SPAWN_REINFORCEMENTS,
            Self::StepHeight => &STEP_HEIGHT,
            Self::SubmergedMiningSpeed => &SUBMERGED_MINING_SPEED,
            Self::SweepingDamageRatio => &SWEEPING_DAMAGE_RATIO,
            Self::TemptRange => &TEMPT_RANGE,
            Self::WaterMovementEfficiency => &WATER_MOVEMENT_EFFICIENCY,
            Self::WaypointTransmitRange => &WAYPOINT_TRANSMIT_RANGE,
            Self::WaypointReceiveRange => &WAYPOINT_RECEIVE_RANGE,
        }
    }
}
