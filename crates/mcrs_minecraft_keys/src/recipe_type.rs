// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const CRAFTING: StaticResourceLocation = rl!("minecraft:crafting");
pub const SMELTING: StaticResourceLocation = rl!("minecraft:smelting");
pub const BLASTING: StaticResourceLocation = rl!("minecraft:blasting");
pub const SMOKING: StaticResourceLocation = rl!("minecraft:smoking");
pub const CAMPFIRE_COOKING: StaticResourceLocation = rl!("minecraft:campfire_cooking");
pub const STONECUTTING: StaticResourceLocation = rl!("minecraft:stonecutting");
pub const SMITHING: StaticResourceLocation = rl!("minecraft:smithing");
pub const BREWING: StaticResourceLocation = rl!("minecraft:brewing");

pub const ENTRIES: &[StaticResourceLocation] = &[
    CRAFTING,
    SMELTING,
    BLASTING,
    SMOKING,
    CAMPFIRE_COOKING,
    STONECUTTING,
    SMITHING,
    BREWING,
];
