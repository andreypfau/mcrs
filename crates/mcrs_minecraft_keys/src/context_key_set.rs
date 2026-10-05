// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ADVANCEMENT_ENTITY: Id<crate::ContextKeySet> = Id::from_static(18);
pub const ADVANCEMENT_LOCATION: Id<crate::ContextKeySet> = Id::from_static(19);
pub const ADVANCEMENT_REWARD: Id<crate::ContextKeySet> = Id::from_static(17);
pub const ARCHAEOLOGY: Id<crate::ContextKeySet> = Id::from_static(13);
pub const BARTER: Id<crate::ContextKeySet> = Id::from_static(15);
pub const BLOCK: Id<crate::ContextKeySet> = Id::from_static(21);
pub const BLOCK_INTERACT: Id<crate::ContextKeySet> = Id::from_static(24);
pub const BLOCK_USE: Id<crate::ContextKeySet> = Id::from_static(20);
pub const CHEST: Id<crate::ContextKeySet> = Id::from_static(2);
pub const COMMAND: Id<crate::ContextKeySet> = Id::from_static(3);
pub const COMMAND_COMPUTE_DEFAULT: Id<crate::ContextKeySet> = Id::from_static(5);
pub const COMMAND_COMPUTE_ENTITY: Id<crate::ContextKeySet> = Id::from_static(7);
pub const COMMAND_COMPUTE_POSITION: Id<crate::ContextKeySet> = Id::from_static(6);
pub const COMMAND_SLOT_SOURCE: Id<crate::ContextKeySet> = Id::from_static(4);
pub const CONTAINER_PROCESS: Id<crate::ContextKeySet> = Id::from_static(25);
pub const EMPTY: Id<crate::ContextKeySet> = Id::from_static(0);
pub const ENCHANTED_DAMAGE: Id<crate::ContextKeySet> = Id::from_static(26);
pub const ENCHANTED_ENTITY: Id<crate::ContextKeySet> = Id::from_static(29);
pub const ENCHANTED_ITEM: Id<crate::ContextKeySet> = Id::from_static(27);
pub const ENCHANTED_LOCATION: Id<crate::ContextKeySet> = Id::from_static(28);
pub const ENTITY: Id<crate::ContextKeySet> = Id::from_static(11);
pub const ENTITY_INTERACT: Id<crate::ContextKeySet> = Id::from_static(23);
pub const EQUIPMENT: Id<crate::ContextKeySet> = Id::from_static(12);
pub const FISHING: Id<crate::ContextKeySet> = Id::from_static(10);
pub const GENERIC: Id<crate::ContextKeySet> = Id::from_static(1);
pub const GIFT: Id<crate::ContextKeySet> = Id::from_static(14);
pub const HIT_BLOCK: Id<crate::ContextKeySet> = Id::from_static(30);
pub const SELECTOR: Id<crate::ContextKeySet> = Id::from_static(8);
pub const SHEARING: Id<crate::ContextKeySet> = Id::from_static(22);
pub const VAULT: Id<crate::ContextKeySet> = Id::from_static(16);
pub const VILLAGER_TRADE: Id<crate::ContextKeySet> = Id::from_static(9);

pub const NAMES: &[&str] = &[
    "minecraft:empty",
    "minecraft:generic",
    "minecraft:chest",
    "minecraft:command",
    "minecraft:command_slot_source",
    "minecraft:command_compute_default",
    "minecraft:command_compute_position",
    "minecraft:command_compute_entity",
    "minecraft:selector",
    "minecraft:villager_trade",
    "minecraft:fishing",
    "minecraft:entity",
    "minecraft:equipment",
    "minecraft:archaeology",
    "minecraft:gift",
    "minecraft:barter",
    "minecraft:vault",
    "minecraft:advancement_reward",
    "minecraft:advancement_entity",
    "minecraft:advancement_location",
    "minecraft:block_use",
    "minecraft:block",
    "minecraft:shearing",
    "minecraft:entity_interact",
    "minecraft:block_interact",
    "minecraft:container_process",
    "minecraft:enchanted_damage",
    "minecraft:enchanted_item",
    "minecraft:enchanted_location",
    "minecraft:enchanted_entity",
    "minecraft:hit_block",
];
