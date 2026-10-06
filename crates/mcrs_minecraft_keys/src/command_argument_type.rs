// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const BRIGADIER_BOOL: StaticResourceLocation = rl!("brigadier:bool");
pub const BRIGADIER_FLOAT: StaticResourceLocation = rl!("brigadier:float");
pub const BRIGADIER_DOUBLE: StaticResourceLocation = rl!("brigadier:double");
pub const BRIGADIER_INTEGER: StaticResourceLocation = rl!("brigadier:integer");
pub const BRIGADIER_LONG: StaticResourceLocation = rl!("brigadier:long");
pub const BRIGADIER_STRING: StaticResourceLocation = rl!("brigadier:string");
pub const ENTITY: StaticResourceLocation = rl!("minecraft:entity");
pub const GAME_PROFILE: StaticResourceLocation = rl!("minecraft:game_profile");
pub const BLOCK_POS: StaticResourceLocation = rl!("minecraft:block_pos");
pub const COLUMN_POS: StaticResourceLocation = rl!("minecraft:column_pos");
pub const VEC3: StaticResourceLocation = rl!("minecraft:vec3");
pub const VEC2: StaticResourceLocation = rl!("minecraft:vec2");
pub const BLOCK_STATE: StaticResourceLocation = rl!("minecraft:block_state");
pub const BLOCK_PREDICATE: StaticResourceLocation = rl!("minecraft:block_predicate");
pub const ITEM_STACK: StaticResourceLocation = rl!("minecraft:item_stack");
pub const ITEM_PREDICATE: StaticResourceLocation = rl!("minecraft:item_predicate");
pub const TEAM_COLOR: StaticResourceLocation = rl!("minecraft:team_color");
pub const HEX_COLOR: StaticResourceLocation = rl!("minecraft:hex_color");
pub const COMPONENT: StaticResourceLocation = rl!("minecraft:component");
pub const STYLE: StaticResourceLocation = rl!("minecraft:style");
pub const MESSAGE: StaticResourceLocation = rl!("minecraft:message");
pub const NBT_COMPOUND_TAG: StaticResourceLocation = rl!("minecraft:nbt_compound_tag");
pub const NBT_TAG: StaticResourceLocation = rl!("minecraft:nbt_tag");
pub const NBT_PATH: StaticResourceLocation = rl!("minecraft:nbt_path");
pub const OBJECTIVE: StaticResourceLocation = rl!("minecraft:objective");
pub const OBJECTIVE_CRITERIA: StaticResourceLocation = rl!("minecraft:objective_criteria");
pub const OPERATION: StaticResourceLocation = rl!("minecraft:operation");
pub const PARTICLE: StaticResourceLocation = rl!("minecraft:particle");
pub const ANGLE: StaticResourceLocation = rl!("minecraft:angle");
pub const ROTATION: StaticResourceLocation = rl!("minecraft:rotation");
pub const SCOREBOARD_SLOT: StaticResourceLocation = rl!("minecraft:scoreboard_slot");
pub const SCORE_HOLDER: StaticResourceLocation = rl!("minecraft:score_holder");
pub const SWIZZLE: StaticResourceLocation = rl!("minecraft:swizzle");
pub const TEAM: StaticResourceLocation = rl!("minecraft:team");
pub const ITEM_SLOT: StaticResourceLocation = rl!("minecraft:item_slot");
pub const ITEM_SLOTS: StaticResourceLocation = rl!("minecraft:item_slots");
pub const RESOURCE_LOCATION: StaticResourceLocation = rl!("minecraft:resource_location");
pub const FUNCTION: StaticResourceLocation = rl!("minecraft:function");
pub const ENTITY_ANCHOR: StaticResourceLocation = rl!("minecraft:entity_anchor");
pub const INT_RANGE: StaticResourceLocation = rl!("minecraft:int_range");
pub const FLOAT_RANGE: StaticResourceLocation = rl!("minecraft:float_range");
pub const DIMENSION: StaticResourceLocation = rl!("minecraft:dimension");
pub const GAMEMODE: StaticResourceLocation = rl!("minecraft:gamemode");
pub const TIME: StaticResourceLocation = rl!("minecraft:time");
pub const RESOURCE_OR_TAG: StaticResourceLocation = rl!("minecraft:resource_or_tag");
pub const RESOURCE_OR_TAG_KEY: StaticResourceLocation = rl!("minecraft:resource_or_tag_key");
pub const RESOURCE: StaticResourceLocation = rl!("minecraft:resource");
pub const RESOURCE_KEY: StaticResourceLocation = rl!("minecraft:resource_key");
pub const RESOURCE_SELECTOR: StaticResourceLocation = rl!("minecraft:resource_selector");
pub const TEMPLATE_MIRROR: StaticResourceLocation = rl!("minecraft:template_mirror");
pub const TEMPLATE_ROTATION: StaticResourceLocation = rl!("minecraft:template_rotation");
pub const HEIGHTMAP: StaticResourceLocation = rl!("minecraft:heightmap");
pub const LOOT_TABLE: StaticResourceLocation = rl!("minecraft:loot_table");
pub const LOOT_PREDICATE: StaticResourceLocation = rl!("minecraft:loot_predicate");
pub const LOOT_MODIFIER: StaticResourceLocation = rl!("minecraft:loot_modifier");
pub const CONTEXT_FLOAT_PROVIDER: StaticResourceLocation = rl!("minecraft:context_float_provider");
pub const CONTEXT_INT_PROVIDER: StaticResourceLocation = rl!("minecraft:context_int_provider");
pub const SLOT_SOURCE: StaticResourceLocation = rl!("minecraft:slot_source");
pub const DIALOG: StaticResourceLocation = rl!("minecraft:dialog");
pub const FEATURE: StaticResourceLocation = rl!("minecraft:feature");
pub const SWING_ANIMATION: StaticResourceLocation = rl!("minecraft:swing_animation");
pub const UUID: StaticResourceLocation = rl!("minecraft:uuid");

pub const ENTRIES: &[StaticResourceLocation] = &[
    BRIGADIER_BOOL,
    BRIGADIER_FLOAT,
    BRIGADIER_DOUBLE,
    BRIGADIER_INTEGER,
    BRIGADIER_LONG,
    BRIGADIER_STRING,
    ENTITY,
    GAME_PROFILE,
    BLOCK_POS,
    COLUMN_POS,
    VEC3,
    VEC2,
    BLOCK_STATE,
    BLOCK_PREDICATE,
    ITEM_STACK,
    ITEM_PREDICATE,
    TEAM_COLOR,
    HEX_COLOR,
    COMPONENT,
    STYLE,
    MESSAGE,
    NBT_COMPOUND_TAG,
    NBT_TAG,
    NBT_PATH,
    OBJECTIVE,
    OBJECTIVE_CRITERIA,
    OPERATION,
    PARTICLE,
    ANGLE,
    ROTATION,
    SCOREBOARD_SLOT,
    SCORE_HOLDER,
    SWIZZLE,
    TEAM,
    ITEM_SLOT,
    ITEM_SLOTS,
    RESOURCE_LOCATION,
    FUNCTION,
    ENTITY_ANCHOR,
    INT_RANGE,
    FLOAT_RANGE,
    DIMENSION,
    GAMEMODE,
    TIME,
    RESOURCE_OR_TAG,
    RESOURCE_OR_TAG_KEY,
    RESOURCE,
    RESOURCE_KEY,
    RESOURCE_SELECTOR,
    TEMPLATE_MIRROR,
    TEMPLATE_ROTATION,
    HEIGHTMAP,
    LOOT_TABLE,
    LOOT_PREDICATE,
    LOOT_MODIFIER,
    CONTEXT_FLOAT_PROVIDER,
    CONTEXT_INT_PROVIDER,
    SLOT_SOURCE,
    DIALOG,
    FEATURE,
    SWING_ANIMATION,
    UUID,
];
