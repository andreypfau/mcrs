use mcrs_minecraft_core::{ResourceLocation, rl};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attribute {
    pub identifier: ResourceLocation<&'static str>,
    pub protocol_id: u32,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub syncable: bool,
}

impl Attribute {
    pub const fn new(
        identifier: ResourceLocation<&'static str>,
        protocol_id: u32,
        default: f64,
        min: f64,
        max: f64,
        syncable: bool,
    ) -> Self {
        Self {
            identifier,
            protocol_id,
            default,
            min,
            max,
            syncable,
        }
    }
}

pub static AIR_DRAG_MODIFIER: Attribute = Attribute::new(
    rl!("minecraft:air_drag_modifier"),
    0,
    1.0,
    0.0,
    2048.0,
    true,
);
pub static ARMOR: Attribute = Attribute::new(rl!("minecraft:armor"), 1, 0.0, 0.0, 30.0, true);
pub static ARMOR_TOUGHNESS: Attribute =
    Attribute::new(rl!("minecraft:armor_toughness"), 2, 0.0, 0.0, 20.0, true);
pub static ATTACK_DAMAGE: Attribute =
    Attribute::new(rl!("minecraft:attack_damage"), 3, 2.0, 0.0, 2048.0, false);
pub static ATTACK_KNOCKBACK: Attribute =
    Attribute::new(rl!("minecraft:attack_knockback"), 4, 0.0, 0.0, 5.0, false);
pub static ATTACK_SPEED: Attribute =
    Attribute::new(rl!("minecraft:attack_speed"), 5, 4.0, 0.0, 1024.0, true);
pub static BELOW_NAME_DISTANCE: Attribute = Attribute::new(
    rl!("minecraft:below_name_distance"),
    6,
    10.0,
    0.0,
    512.0,
    true,
);
pub static BLOCK_BREAK_SPEED: Attribute = Attribute::new(
    rl!("minecraft:block_break_speed"),
    7,
    1.0,
    0.0,
    1024.0,
    true,
);
pub static BLOCK_INTERACTION_RANGE: Attribute = Attribute::new(
    rl!("minecraft:block_interaction_range"),
    8,
    4.5,
    0.0,
    64.0,
    true,
);
pub static BOUNCINESS: Attribute =
    Attribute::new(rl!("minecraft:bounciness"), 9, 0.0, 0.0, 1.0, true);
pub static BURNING_TIME: Attribute =
    Attribute::new(rl!("minecraft:burning_time"), 10, 1.0, 0.0, 1024.0, true);
pub static CAMERA_DISTANCE: Attribute =
    Attribute::new(rl!("minecraft:camera_distance"), 11, 4.0, 0.0, 32.0, true);
pub static EXPLOSION_KNOCKBACK_RESISTANCE: Attribute = Attribute::new(
    rl!("minecraft:explosion_knockback_resistance"),
    12,
    0.0,
    0.0,
    1.0,
    true,
);
pub static ENTITY_INTERACTION_RANGE: Attribute = Attribute::new(
    rl!("minecraft:entity_interaction_range"),
    13,
    3.0,
    0.0,
    64.0,
    true,
);
pub static FALL_DAMAGE_MULTIPLIER: Attribute = Attribute::new(
    rl!("minecraft:fall_damage_multiplier"),
    14,
    1.0,
    0.0,
    100.0,
    true,
);
pub static FLYING_SPEED: Attribute =
    Attribute::new(rl!("minecraft:flying_speed"), 15, 0.4, 0.0, 1024.0, true);
pub static FOLLOW_RANGE: Attribute =
    Attribute::new(rl!("minecraft:follow_range"), 16, 32.0, 0.0, 2048.0, false);
pub static FRICTION_MODIFIER: Attribute = Attribute::new(
    rl!("minecraft:friction_modifier"),
    17,
    1.0,
    0.0,
    2048.0,
    true,
);
pub static GRAVITY: Attribute = Attribute::new(rl!("minecraft:gravity"), 18, 0.08, -1.0, 1.0, true);
pub static JUMP_STRENGTH: Attribute = Attribute::new(
    rl!("minecraft:jump_strength"),
    19,
    0.42_f32 as f64,
    0.0,
    32.0,
    true,
);
pub static KNOCKBACK_RESISTANCE: Attribute = Attribute::new(
    rl!("minecraft:knockback_resistance"),
    20,
    0.0,
    -2.0,
    1.0,
    false,
);
pub static LUCK: Attribute = Attribute::new(rl!("minecraft:luck"), 21, 0.0, -1024.0, 1024.0, true);
pub static MAX_ABSORPTION: Attribute =
    Attribute::new(rl!("minecraft:max_absorption"), 22, 0.0, 0.0, 2048.0, true);
pub static MAX_HEALTH: Attribute =
    Attribute::new(rl!("minecraft:max_health"), 23, 20.0, 1.0, 1024.0, true);
pub static MINING_EFFICIENCY: Attribute = Attribute::new(
    rl!("minecraft:mining_efficiency"),
    24,
    0.0,
    0.0,
    1024.0,
    true,
);
pub static MOVEMENT_EFFICIENCY: Attribute = Attribute::new(
    rl!("minecraft:movement_efficiency"),
    25,
    0.0,
    0.0,
    1.0,
    true,
);
pub static MOVEMENT_SPEED: Attribute =
    Attribute::new(rl!("minecraft:movement_speed"), 26, 0.7, 0.0, 1024.0, true);
pub static NAME_TAG_DISTANCE: Attribute = Attribute::new(
    rl!("minecraft:name_tag_distance"),
    27,
    64.0,
    0.0,
    512.0,
    true,
);
pub static OXYGEN_BONUS: Attribute =
    Attribute::new(rl!("minecraft:oxygen_bonus"), 28, 0.0, 0.0, 1024.0, true);
pub static SAFE_FALL_DISTANCE: Attribute = Attribute::new(
    rl!("minecraft:safe_fall_distance"),
    29,
    3.0,
    -1024.0,
    1024.0,
    true,
);
pub static SCALE: Attribute = Attribute::new(rl!("minecraft:scale"), 30, 1.0, 0.0625, 16.0, true);
pub static SNEAKING_SPEED: Attribute =
    Attribute::new(rl!("minecraft:sneaking_speed"), 31, 0.3, 0.0, 1.0, true);
pub static SPAWN_REINFORCEMENTS: Attribute = Attribute::new(
    rl!("minecraft:spawn_reinforcements"),
    32,
    0.0,
    0.0,
    1.0,
    false,
);
pub static STEP_HEIGHT: Attribute =
    Attribute::new(rl!("minecraft:step_height"), 33, 0.6, 0.0, 10.0, true);
pub static SUBMERGED_MINING_SPEED: Attribute = Attribute::new(
    rl!("minecraft:submerged_mining_speed"),
    34,
    0.2,
    0.0,
    20.0,
    true,
);
pub static SWEEPING_DAMAGE_RATIO: Attribute = Attribute::new(
    rl!("minecraft:sweeping_damage_ratio"),
    35,
    0.0,
    0.0,
    1.0,
    true,
);
pub static TEMPT_RANGE: Attribute =
    Attribute::new(rl!("minecraft:tempt_range"), 36, 10.0, 0.0, 2048.0, false);
pub static WATER_MOVEMENT_EFFICIENCY: Attribute = Attribute::new(
    rl!("minecraft:water_movement_efficiency"),
    37,
    0.0,
    0.0,
    1.0,
    true,
);
pub static WAYPOINT_TRANSMIT_RANGE: Attribute = Attribute::new(
    rl!("minecraft:waypoint_transmit_range"),
    38,
    0.0,
    0.0,
    60000000.0,
    false,
);
pub static WAYPOINT_RECEIVE_RANGE: Attribute = Attribute::new(
    rl!("minecraft:waypoint_receive_range"),
    39,
    0.0,
    0.0,
    60000000.0,
    false,
);

pub static ALL: [&Attribute; 40] = [
    &AIR_DRAG_MODIFIER,
    &ARMOR,
    &ARMOR_TOUGHNESS,
    &ATTACK_DAMAGE,
    &ATTACK_KNOCKBACK,
    &ATTACK_SPEED,
    &BELOW_NAME_DISTANCE,
    &BLOCK_BREAK_SPEED,
    &BLOCK_INTERACTION_RANGE,
    &BOUNCINESS,
    &BURNING_TIME,
    &CAMERA_DISTANCE,
    &EXPLOSION_KNOCKBACK_RESISTANCE,
    &ENTITY_INTERACTION_RANGE,
    &FALL_DAMAGE_MULTIPLIER,
    &FLYING_SPEED,
    &FOLLOW_RANGE,
    &FRICTION_MODIFIER,
    &GRAVITY,
    &JUMP_STRENGTH,
    &KNOCKBACK_RESISTANCE,
    &LUCK,
    &MAX_ABSORPTION,
    &MAX_HEALTH,
    &MINING_EFFICIENCY,
    &MOVEMENT_EFFICIENCY,
    &MOVEMENT_SPEED,
    &NAME_TAG_DISTANCE,
    &OXYGEN_BONUS,
    &SAFE_FALL_DISTANCE,
    &SCALE,
    &SNEAKING_SPEED,
    &SPAWN_REINFORCEMENTS,
    &STEP_HEIGHT,
    &SUBMERGED_MINING_SPEED,
    &SWEEPING_DAMAGE_RATIO,
    &TEMPT_RANGE,
    &WATER_MOVEMENT_EFFICIENCY,
    &WAYPOINT_TRANSMIT_RANGE,
    &WAYPOINT_RECEIVE_RANGE,
];
