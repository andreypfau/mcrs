// The `Sync` check of a villager trade walks text through loot `Holder`s, whose `V::Registry`
// projection keeps that cycle from being coinductive, so it goes deeper than the default limit.
#![recursion_limit = "256"]
pub mod chat_type;
#[rustfmt::skip]
pub mod keys;
pub mod dimension;
pub mod enchantment_provider;
pub mod entity;
pub mod item;
pub mod packs;
pub mod registries;
pub mod registry_load;
pub mod sulfur_cube_archetype;
pub mod test_types;
pub mod variant;
pub mod villager_trade;
pub mod worldgen;
