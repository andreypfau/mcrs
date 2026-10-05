// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BANDLANDS: Id<crate::MaterialRuleType> = Id::from_static(1);
pub const BLOCK: Id<crate::MaterialRuleType> = Id::from_static(0);
pub const CONDITION: Id<crate::MaterialRuleType> = Id::from_static(3);
pub const ORE_VEIN: Id<crate::MaterialRuleType> = Id::from_static(4);
pub const SEQUENCE: Id<crate::MaterialRuleType> = Id::from_static(2);

pub const NAMES: &[&str] = &[
    "minecraft:block",
    "minecraft:bandlands",
    "minecraft:sequence",
    "minecraft:condition",
    "minecraft:ore_vein",
];
