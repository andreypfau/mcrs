// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ABS: Id<crate::ContextIntProviderType> = Id::from_static(0);
pub const ADD: Id<crate::ContextIntProviderType> = Id::from_static(20);
pub const AVG: Id<crate::ContextIntProviderType> = Id::from_static(1);
pub const BINOMIAL: Id<crate::ContextIntProviderType> = Id::from_static(2);
pub const CONDITIONAL: Id<crate::ContextIntProviderType> = Id::from_static(3);
pub const CONSTANT: Id<crate::ContextIntProviderType> = Id::from_static(4);
pub const DIV: Id<crate::ContextIntProviderType> = Id::from_static(13);
pub const ENVIRONMENT_ATTRIBUTE: Id<crate::ContextIntProviderType> = Id::from_static(6);
pub const FLOOR_DIV: Id<crate::ContextIntProviderType> = Id::from_static(11);
pub const FLOOR_MOD: Id<crate::ContextIntProviderType> = Id::from_static(10);
pub const FROM_FLOAT: Id<crate::ContextIntProviderType> = Id::from_static(7);
pub const MAX: Id<crate::ContextIntProviderType> = Id::from_static(8);
pub const MIN: Id<crate::ContextIntProviderType> = Id::from_static(9);
pub const MOD: Id<crate::ContextIntProviderType> = Id::from_static(12);
pub const MUL: Id<crate::ContextIntProviderType> = Id::from_static(17);
pub const NEGATE: Id<crate::ContextIntProviderType> = Id::from_static(14);
pub const NUMBER_DISPATCHER: Id<crate::ContextIntProviderType> = Id::from_static(15);
pub const POW: Id<crate::ContextIntProviderType> = Id::from_static(16);
pub const SCORE: Id<crate::ContextIntProviderType> = Id::from_static(18);
pub const STORAGE: Id<crate::ContextIntProviderType> = Id::from_static(19);
pub const SUB: Id<crate::ContextIntProviderType> = Id::from_static(5);
pub const UNIFORM: Id<crate::ContextIntProviderType> = Id::from_static(21);
pub const WEIGHTED_LIST: Id<crate::ContextIntProviderType> = Id::from_static(22);

pub const NAMES: &[&str] = &[
    "minecraft:abs",
    "minecraft:avg",
    "minecraft:binomial",
    "minecraft:conditional",
    "minecraft:constant",
    "minecraft:sub",
    "minecraft:environment_attribute",
    "minecraft:from_float",
    "minecraft:max",
    "minecraft:min",
    "minecraft:floor_mod",
    "minecraft:floor_div",
    "minecraft:mod",
    "minecraft:div",
    "minecraft:negate",
    "minecraft:number_dispatcher",
    "minecraft:pow",
    "minecraft:mul",
    "minecraft:score",
    "minecraft:storage",
    "minecraft:add",
    "minecraft:uniform",
    "minecraft:weighted_list",
];
