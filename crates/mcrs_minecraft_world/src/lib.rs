#![allow(
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod chat_type;
#[rustfmt::skip]
pub mod keys;
#[cfg(feature = "bevy")]
pub mod data_pack;
pub mod dimension;
pub mod enchantment_provider;
pub mod entity;
pub mod item;
pub mod packs;
#[cfg(feature = "bevy")]
mod plugin;
pub mod registries;
#[cfg(feature = "bevy")]
pub mod resolvers;
// The save on disk is native-only; the browser receives world state over the network.
#[cfg(not(target_family = "wasm"))]
pub mod save;
pub mod sulfur_cube_archetype;
pub mod test_types;
pub mod variant;
pub mod villager_trade;
pub mod worldgen;

#[cfg(feature = "bevy")]
pub use plugin::{LoadedRegistryAssets, MinecraftWorldPlugin, transition_to_playing};
