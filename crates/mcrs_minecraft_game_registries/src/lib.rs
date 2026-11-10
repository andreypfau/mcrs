// The `Sync` check of a villager trade walks text through loot `Holder`s, whose `V::Registry`
// projection keeps that cycle from being coinductive, so it goes deeper than the default limit.
#![recursion_limit = "256"]
pub mod chat_type;
#[rustfmt::skip]
pub mod chat_type_keys;
#[rustfmt::skip]
pub mod enchantment_provider_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod sulfur_cube_archetype_keys;
#[rustfmt::skip]
pub mod test_environment_definition_type_keys;
#[rustfmt::skip]
pub mod test_environment_keys;
#[rustfmt::skip]
pub mod test_instance_keys;
#[rustfmt::skip]
pub mod test_instance_type_keys;
#[rustfmt::skip]
pub mod trade_set_keys;
#[rustfmt::skip]
pub mod villager_trade_keys;
#[rustfmt::skip]
pub mod villager_trade_tag_keys;
#[rustfmt::skip]
pub mod world_preset_keys;
#[rustfmt::skip]
pub mod world_preset_tag_keys;
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
