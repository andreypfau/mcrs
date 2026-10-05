// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BRIGADIER_BOOL: Id<crate::CommandArgumentType> = Id::from_static(0);
pub const BRIGADIER_DOUBLE: Id<crate::CommandArgumentType> = Id::from_static(2);
pub const BRIGADIER_FLOAT: Id<crate::CommandArgumentType> = Id::from_static(1);
pub const BRIGADIER_INTEGER: Id<crate::CommandArgumentType> = Id::from_static(3);
pub const BRIGADIER_LONG: Id<crate::CommandArgumentType> = Id::from_static(4);
pub const BRIGADIER_STRING: Id<crate::CommandArgumentType> = Id::from_static(5);
pub const ANGLE: Id<crate::CommandArgumentType> = Id::from_static(28);
pub const BLOCK_POS: Id<crate::CommandArgumentType> = Id::from_static(8);
pub const BLOCK_PREDICATE: Id<crate::CommandArgumentType> = Id::from_static(13);
pub const BLOCK_STATE: Id<crate::CommandArgumentType> = Id::from_static(12);
pub const COLUMN_POS: Id<crate::CommandArgumentType> = Id::from_static(9);
pub const COMPONENT: Id<crate::CommandArgumentType> = Id::from_static(18);
pub const CONTEXT_FLOAT_PROVIDER: Id<crate::CommandArgumentType> = Id::from_static(55);
pub const CONTEXT_INT_PROVIDER: Id<crate::CommandArgumentType> = Id::from_static(56);
pub const DIALOG: Id<crate::CommandArgumentType> = Id::from_static(58);
pub const DIMENSION: Id<crate::CommandArgumentType> = Id::from_static(41);
pub const ENTITY: Id<crate::CommandArgumentType> = Id::from_static(6);
pub const ENTITY_ANCHOR: Id<crate::CommandArgumentType> = Id::from_static(38);
pub const FEATURE: Id<crate::CommandArgumentType> = Id::from_static(59);
pub const FLOAT_RANGE: Id<crate::CommandArgumentType> = Id::from_static(40);
pub const FUNCTION: Id<crate::CommandArgumentType> = Id::from_static(37);
pub const GAME_PROFILE: Id<crate::CommandArgumentType> = Id::from_static(7);
pub const GAMEMODE: Id<crate::CommandArgumentType> = Id::from_static(42);
pub const HEIGHTMAP: Id<crate::CommandArgumentType> = Id::from_static(51);
pub const HEX_COLOR: Id<crate::CommandArgumentType> = Id::from_static(17);
pub const INT_RANGE: Id<crate::CommandArgumentType> = Id::from_static(39);
pub const ITEM_PREDICATE: Id<crate::CommandArgumentType> = Id::from_static(15);
pub const ITEM_SLOT: Id<crate::CommandArgumentType> = Id::from_static(34);
pub const ITEM_SLOTS: Id<crate::CommandArgumentType> = Id::from_static(35);
pub const ITEM_STACK: Id<crate::CommandArgumentType> = Id::from_static(14);
pub const LOOT_MODIFIER: Id<crate::CommandArgumentType> = Id::from_static(54);
pub const LOOT_PREDICATE: Id<crate::CommandArgumentType> = Id::from_static(53);
pub const LOOT_TABLE: Id<crate::CommandArgumentType> = Id::from_static(52);
pub const MESSAGE: Id<crate::CommandArgumentType> = Id::from_static(20);
pub const NBT_COMPOUND_TAG: Id<crate::CommandArgumentType> = Id::from_static(21);
pub const NBT_PATH: Id<crate::CommandArgumentType> = Id::from_static(23);
pub const NBT_TAG: Id<crate::CommandArgumentType> = Id::from_static(22);
pub const OBJECTIVE: Id<crate::CommandArgumentType> = Id::from_static(24);
pub const OBJECTIVE_CRITERIA: Id<crate::CommandArgumentType> = Id::from_static(25);
pub const OPERATION: Id<crate::CommandArgumentType> = Id::from_static(26);
pub const PARTICLE: Id<crate::CommandArgumentType> = Id::from_static(27);
pub const RESOURCE: Id<crate::CommandArgumentType> = Id::from_static(46);
pub const RESOURCE_KEY: Id<crate::CommandArgumentType> = Id::from_static(47);
pub const RESOURCE_LOCATION: Id<crate::CommandArgumentType> = Id::from_static(36);
pub const RESOURCE_OR_TAG: Id<crate::CommandArgumentType> = Id::from_static(44);
pub const RESOURCE_OR_TAG_KEY: Id<crate::CommandArgumentType> = Id::from_static(45);
pub const RESOURCE_SELECTOR: Id<crate::CommandArgumentType> = Id::from_static(48);
pub const ROTATION: Id<crate::CommandArgumentType> = Id::from_static(29);
pub const SCORE_HOLDER: Id<crate::CommandArgumentType> = Id::from_static(31);
pub const SCOREBOARD_SLOT: Id<crate::CommandArgumentType> = Id::from_static(30);
pub const SLOT_SOURCE: Id<crate::CommandArgumentType> = Id::from_static(57);
pub const STYLE: Id<crate::CommandArgumentType> = Id::from_static(19);
pub const SWING_ANIMATION: Id<crate::CommandArgumentType> = Id::from_static(60);
pub const SWIZZLE: Id<crate::CommandArgumentType> = Id::from_static(32);
pub const TEAM: Id<crate::CommandArgumentType> = Id::from_static(33);
pub const TEAM_COLOR: Id<crate::CommandArgumentType> = Id::from_static(16);
pub const TEMPLATE_MIRROR: Id<crate::CommandArgumentType> = Id::from_static(49);
pub const TEMPLATE_ROTATION: Id<crate::CommandArgumentType> = Id::from_static(50);
pub const TIME: Id<crate::CommandArgumentType> = Id::from_static(43);
pub const UUID: Id<crate::CommandArgumentType> = Id::from_static(61);
pub const VEC2: Id<crate::CommandArgumentType> = Id::from_static(11);
pub const VEC3: Id<crate::CommandArgumentType> = Id::from_static(10);

pub const NAMES: &[&str] = &[
    "brigadier:bool",
    "brigadier:float",
    "brigadier:double",
    "brigadier:integer",
    "brigadier:long",
    "brigadier:string",
    "minecraft:entity",
    "minecraft:game_profile",
    "minecraft:block_pos",
    "minecraft:column_pos",
    "minecraft:vec3",
    "minecraft:vec2",
    "minecraft:block_state",
    "minecraft:block_predicate",
    "minecraft:item_stack",
    "minecraft:item_predicate",
    "minecraft:team_color",
    "minecraft:hex_color",
    "minecraft:component",
    "minecraft:style",
    "minecraft:message",
    "minecraft:nbt_compound_tag",
    "minecraft:nbt_tag",
    "minecraft:nbt_path",
    "minecraft:objective",
    "minecraft:objective_criteria",
    "minecraft:operation",
    "minecraft:particle",
    "minecraft:angle",
    "minecraft:rotation",
    "minecraft:scoreboard_slot",
    "minecraft:score_holder",
    "minecraft:swizzle",
    "minecraft:team",
    "minecraft:item_slot",
    "minecraft:item_slots",
    "minecraft:resource_location",
    "minecraft:function",
    "minecraft:entity_anchor",
    "minecraft:int_range",
    "minecraft:float_range",
    "minecraft:dimension",
    "minecraft:gamemode",
    "minecraft:time",
    "minecraft:resource_or_tag",
    "minecraft:resource_or_tag_key",
    "minecraft:resource",
    "minecraft:resource_key",
    "minecraft:resource_selector",
    "minecraft:template_mirror",
    "minecraft:template_rotation",
    "minecraft:heightmap",
    "minecraft:loot_table",
    "minecraft:loot_predicate",
    "minecraft:loot_modifier",
    "minecraft:context_float_provider",
    "minecraft:context_int_provider",
    "minecraft:slot_source",
    "minecraft:dialog",
    "minecraft:feature",
    "minecraft:swing_animation",
    "minecraft:uuid",
];
