// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALWAYS_TRUE: Id<crate::PosRuleTest> = Id::from_static(0);
pub const AXIS_ALIGNED_LINEAR_POS: Id<crate::PosRuleTest> = Id::from_static(2);
pub const LINEAR_POS: Id<crate::PosRuleTest> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:always_true",
    "minecraft:linear_pos",
    "minecraft:axis_aligned_linear_pos",
];
