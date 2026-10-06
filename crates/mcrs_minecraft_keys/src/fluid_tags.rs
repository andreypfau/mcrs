// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const AXOLOTL_TRIES_TO_FIND: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:axolotl_tries_to_find"));
pub const BUBBLE_COLUMN_CAN_OCCUPY: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:bubble_column_can_occupy"));
pub const DOLPHIN_TRIES_TO_FIND: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:dolphin_tries_to_find"));
pub const ENTITY_FLOATABLE: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:entity_floatable"));
pub const FROG_TRIES_TO_FIND_LAND_NEAR: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:frog_tries_to_find_land_near"));
pub const LAVA: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:lava"));
pub const SUPPORTS_FROGSPAWN: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:supports_frogspawn"));
pub const SUPPORTS_LILY_PAD: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:supports_lily_pad"));
pub const SUPPORTS_SUGAR_CANE_ADJACENTLY: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:supports_sugar_cane_adjacently"));
pub const WATER: TagKey<crate::Fluid, &'static str> = TagKey::new(rl!("minecraft:water"));
