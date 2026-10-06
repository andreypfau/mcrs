// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const BY_COST: StaticResourceLocation = rl!("minecraft:by_cost");
pub const BY_COST_WITH_DIFFICULTY: StaticResourceLocation = rl!("minecraft:by_cost_with_difficulty");
pub const SINGLE: StaticResourceLocation = rl!("minecraft:single");

pub const ENTRIES: &[StaticResourceLocation] = &[
    BY_COST,
    BY_COST_WITH_DIFFICULTY,
    SINGLE,
];
