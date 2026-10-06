// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const ENDERMAN_LOOT_DROP: ResourceKey<crate::enchantment_provider::EnchantmentProvider, &'static str> = ResourceKey::new(rl!("minecraft:enderman_loot_drop"));
pub const MOB_SPAWN_EQUIPMENT: ResourceKey<crate::enchantment_provider::EnchantmentProvider, &'static str> = ResourceKey::new(rl!("minecraft:mob_spawn_equipment"));
pub const PILLAGER_SPAWN_CROSSBOW: ResourceKey<crate::enchantment_provider::EnchantmentProvider, &'static str> = ResourceKey::new(rl!("minecraft:pillager_spawn_crossbow"));
pub const RAID_PILLAGER_POST_WAVE_3: ResourceKey<crate::enchantment_provider::EnchantmentProvider, &'static str> = ResourceKey::new(rl!("minecraft:raid/pillager_post_wave_3"));
pub const RAID_PILLAGER_POST_WAVE_5: ResourceKey<crate::enchantment_provider::EnchantmentProvider, &'static str> = ResourceKey::new(rl!("minecraft:raid/pillager_post_wave_5"));
pub const RAID_VINDICATOR: ResourceKey<crate::enchantment_provider::EnchantmentProvider, &'static str> = ResourceKey::new(rl!("minecraft:raid/vindicator"));
pub const RAID_VINDICATOR_POST_WAVE_5: ResourceKey<crate::enchantment_provider::EnchantmentProvider, &'static str> = ResourceKey::new(rl!("minecraft:raid/vindicator_post_wave_5"));
