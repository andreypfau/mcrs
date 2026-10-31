// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const CRAFTING: ResourceLocation<'static> = rl!("minecraft:crafting");
pub const SMELTING: ResourceLocation<'static> = rl!("minecraft:smelting");
pub const BLASTING: ResourceLocation<'static> = rl!("minecraft:blasting");
pub const SMOKING: ResourceLocation<'static> = rl!("minecraft:smoking");
pub const CAMPFIRE_COOKING: ResourceLocation<'static> = rl!("minecraft:campfire_cooking");
pub const STONECUTTING: ResourceLocation<'static> = rl!("minecraft:stonecutting");
pub const SMITHING: ResourceLocation<'static> = rl!("minecraft:smithing");
pub const BREWING: ResourceLocation<'static> = rl!("minecraft:brewing");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
    CRAFTING,
    SMELTING,
    BLASTING,
    SMOKING,
    CAMPFIRE_COOKING,
    STONECUTTING,
    SMITHING,
    BREWING,
];
