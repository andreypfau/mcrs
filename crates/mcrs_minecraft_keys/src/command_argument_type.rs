// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const BRIGADIER_BOOL: ResourceLocation<'static> = rl!("brigadier:bool");
pub const BRIGADIER_FLOAT: ResourceLocation<'static> = rl!("brigadier:float");
pub const BRIGADIER_DOUBLE: ResourceLocation<'static> = rl!("brigadier:double");
pub const BRIGADIER_INTEGER: ResourceLocation<'static> = rl!("brigadier:integer");
pub const BRIGADIER_LONG: ResourceLocation<'static> = rl!("brigadier:long");
pub const BRIGADIER_STRING: ResourceLocation<'static> = rl!("brigadier:string");
pub const ENTITY: ResourceLocation<'static> = rl!("minecraft:entity");
pub const GAME_PROFILE: ResourceLocation<'static> = rl!("minecraft:game_profile");
pub const BLOCK_POS: ResourceLocation<'static> = rl!("minecraft:block_pos");
pub const COLUMN_POS: ResourceLocation<'static> = rl!("minecraft:column_pos");
pub const VEC3: ResourceLocation<'static> = rl!("minecraft:vec3");
pub const VEC2: ResourceLocation<'static> = rl!("minecraft:vec2");
pub const BLOCK_STATE: ResourceLocation<'static> = rl!("minecraft:block_state");
pub const BLOCK_PREDICATE: ResourceLocation<'static> = rl!("minecraft:block_predicate");
pub const ITEM_STACK: ResourceLocation<'static> = rl!("minecraft:item_stack");
pub const ITEM_PREDICATE: ResourceLocation<'static> = rl!("minecraft:item_predicate");
pub const TEAM_COLOR: ResourceLocation<'static> = rl!("minecraft:team_color");
pub const HEX_COLOR: ResourceLocation<'static> = rl!("minecraft:hex_color");
pub const COMPONENT: ResourceLocation<'static> = rl!("minecraft:component");
pub const STYLE: ResourceLocation<'static> = rl!("minecraft:style");
pub const MESSAGE: ResourceLocation<'static> = rl!("minecraft:message");
pub const NBT_COMPOUND_TAG: ResourceLocation<'static> = rl!("minecraft:nbt_compound_tag");
pub const NBT_TAG: ResourceLocation<'static> = rl!("minecraft:nbt_tag");
pub const NBT_PATH: ResourceLocation<'static> = rl!("minecraft:nbt_path");
pub const OBJECTIVE: ResourceLocation<'static> = rl!("minecraft:objective");
pub const OBJECTIVE_CRITERIA: ResourceLocation<'static> = rl!("minecraft:objective_criteria");
pub const OPERATION: ResourceLocation<'static> = rl!("minecraft:operation");
pub const PARTICLE: ResourceLocation<'static> = rl!("minecraft:particle");
pub const ANGLE: ResourceLocation<'static> = rl!("minecraft:angle");
pub const ROTATION: ResourceLocation<'static> = rl!("minecraft:rotation");
pub const SCOREBOARD_SLOT: ResourceLocation<'static> = rl!("minecraft:scoreboard_slot");
pub const SCORE_HOLDER: ResourceLocation<'static> = rl!("minecraft:score_holder");
pub const SWIZZLE: ResourceLocation<'static> = rl!("minecraft:swizzle");
pub const TEAM: ResourceLocation<'static> = rl!("minecraft:team");
pub const ITEM_SLOT: ResourceLocation<'static> = rl!("minecraft:item_slot");
pub const ITEM_SLOTS: ResourceLocation<'static> = rl!("minecraft:item_slots");
pub const RESOURCE_LOCATION: ResourceLocation<'static> = rl!("minecraft:resource_location");
pub const FUNCTION: ResourceLocation<'static> = rl!("minecraft:function");
pub const ENTITY_ANCHOR: ResourceLocation<'static> = rl!("minecraft:entity_anchor");
pub const INT_RANGE: ResourceLocation<'static> = rl!("minecraft:int_range");
pub const FLOAT_RANGE: ResourceLocation<'static> = rl!("minecraft:float_range");
pub const DIMENSION: ResourceLocation<'static> = rl!("minecraft:dimension");
pub const GAMEMODE: ResourceLocation<'static> = rl!("minecraft:gamemode");
pub const TIME: ResourceLocation<'static> = rl!("minecraft:time");
pub const RESOURCE_OR_TAG: ResourceLocation<'static> = rl!("minecraft:resource_or_tag");
pub const RESOURCE_OR_TAG_KEY: ResourceLocation<'static> = rl!("minecraft:resource_or_tag_key");
pub const RESOURCE: ResourceLocation<'static> = rl!("minecraft:resource");
pub const RESOURCE_KEY: ResourceLocation<'static> = rl!("minecraft:resource_key");
pub const RESOURCE_SELECTOR: ResourceLocation<'static> = rl!("minecraft:resource_selector");
pub const TEMPLATE_MIRROR: ResourceLocation<'static> = rl!("minecraft:template_mirror");
pub const TEMPLATE_ROTATION: ResourceLocation<'static> = rl!("minecraft:template_rotation");
pub const HEIGHTMAP: ResourceLocation<'static> = rl!("minecraft:heightmap");
pub const LOOT_TABLE: ResourceLocation<'static> = rl!("minecraft:loot_table");
pub const LOOT_PREDICATE: ResourceLocation<'static> = rl!("minecraft:loot_predicate");
pub const LOOT_MODIFIER: ResourceLocation<'static> = rl!("minecraft:loot_modifier");
pub const CONTEXT_FLOAT_PROVIDER: ResourceLocation<'static> = rl!("minecraft:context_float_provider");
pub const CONTEXT_INT_PROVIDER: ResourceLocation<'static> = rl!("minecraft:context_int_provider");
pub const SLOT_SOURCE: ResourceLocation<'static> = rl!("minecraft:slot_source");
pub const DIALOG: ResourceLocation<'static> = rl!("minecraft:dialog");
pub const FEATURE: ResourceLocation<'static> = rl!("minecraft:feature");
pub const SWING_ANIMATION: ResourceLocation<'static> = rl!("minecraft:swing_animation");
pub const UUID: ResourceLocation<'static> = rl!("minecraft:uuid");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
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
