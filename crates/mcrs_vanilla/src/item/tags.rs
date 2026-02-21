use crate::item::Item;
use mcrs_core::tag::key::TagKey;

pub const SWORDS: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:swords"));
pub const PICKAXES: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:pickaxes"));
pub const AXES: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:axes"));
pub const SHOVELS: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:shovels"));
pub const HOES: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:hoes"));

/// All item tag keys, for bulk loading.
pub const ALL_ITEM_TAGS: &[TagKey<Item>] = &[SWORDS, PICKAXES, AXES, SHOVELS, HOES];
