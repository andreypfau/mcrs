// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ABS: Id<crate::ContextFloatProviderType> = Id::from_static(0);
pub const ADD: Id<crate::ContextFloatProviderType> = Id::from_static(24);
pub const AVG: Id<crate::ContextFloatProviderType> = Id::from_static(1);
pub const CEIL: Id<crate::ContextFloatProviderType> = Id::from_static(2);
pub const CONDITIONAL: Id<crate::ContextFloatProviderType> = Id::from_static(3);
pub const CONSTANT: Id<crate::ContextFloatProviderType> = Id::from_static(4);
pub const COS: Id<crate::ContextFloatProviderType> = Id::from_static(5);
pub const DIV: Id<crate::ContextFloatProviderType> = Id::from_static(19);
pub const ENCHANTMENT_LEVEL: Id<crate::ContextFloatProviderType> = Id::from_static(7);
pub const ENVIRONMENT_ATTRIBUTE: Id<crate::ContextFloatProviderType> = Id::from_static(8);
pub const FLOOR: Id<crate::ContextFloatProviderType> = Id::from_static(9);
pub const FROM_INT: Id<crate::ContextFloatProviderType> = Id::from_static(10);
pub const LENGTH: Id<crate::ContextFloatProviderType> = Id::from_static(11);
pub const MAX: Id<crate::ContextFloatProviderType> = Id::from_static(12);
pub const MIN: Id<crate::ContextFloatProviderType> = Id::from_static(13);
pub const MOD: Id<crate::ContextFloatProviderType> = Id::from_static(14);
pub const MUL: Id<crate::ContextFloatProviderType> = Id::from_static(18);
pub const NEGATE: Id<crate::ContextFloatProviderType> = Id::from_static(15);
pub const NUMBER_DISPATCHER: Id<crate::ContextFloatProviderType> = Id::from_static(16);
pub const POW: Id<crate::ContextFloatProviderType> = Id::from_static(17);
pub const ROUND: Id<crate::ContextFloatProviderType> = Id::from_static(20);
pub const SIN: Id<crate::ContextFloatProviderType> = Id::from_static(21);
pub const SQRT: Id<crate::ContextFloatProviderType> = Id::from_static(22);
pub const STORAGE: Id<crate::ContextFloatProviderType> = Id::from_static(23);
pub const SUB: Id<crate::ContextFloatProviderType> = Id::from_static(6);
pub const TRUNCATE: Id<crate::ContextFloatProviderType> = Id::from_static(25);
pub const UNIFORM: Id<crate::ContextFloatProviderType> = Id::from_static(26);
pub const WEIGHTED_LIST: Id<crate::ContextFloatProviderType> = Id::from_static(27);

pub const NAMES: &[&str] = &[
    "minecraft:abs",
    "minecraft:avg",
    "minecraft:ceil",
    "minecraft:conditional",
    "minecraft:constant",
    "minecraft:cos",
    "minecraft:sub",
    "minecraft:enchantment_level",
    "minecraft:environment_attribute",
    "minecraft:floor",
    "minecraft:from_int",
    "minecraft:length",
    "minecraft:max",
    "minecraft:min",
    "minecraft:mod",
    "minecraft:negate",
    "minecraft:number_dispatcher",
    "minecraft:pow",
    "minecraft:mul",
    "minecraft:div",
    "minecraft:round",
    "minecraft:sin",
    "minecraft:sqrt",
    "minecraft:storage",
    "minecraft:add",
    "minecraft:truncate",
    "minecraft:uniform",
    "minecraft:weighted_list",
];
