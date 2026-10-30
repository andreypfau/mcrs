//! Block entities in the typed shape the save, the chunk packet and the anvil reader share.
// std: mcrs_minecraft_nbt (`from_bytes_unnamed` reads through `std::io::Cursor`).

// chisle: data components, item components, skull profiles, pot sherds,
// trial-spawner configs and vault runtime state pass through as raw NBT. The
// ceiling is that nothing can read them typed; the upgrade is a typed
// DataComponentMap once something does.

mod banner_layer;
mod bee_occupant;
mod container_data;
mod end_gateway_data;
mod furnace_data;
mod generated_block_entity;
#[cfg(test)]
mod generated_block_entity_tests;
mod hopper_data;
mod jigsaw;
mod saved_item;
mod sign_text;
mod spawn_data;
mod text;
mod uuid;
mod vault_config;

pub use banner_layer::BannerLayer;
pub use bee_occupant::{BEE_MIN_TICKS_IN_HIVE, BeeOccupant};
pub use container_data::ContainerData;
pub use end_gateway_data::EndGatewayData;
pub use furnace_data::FurnaceData;
pub use generated_block_entity::GeneratedBlockEntity;
pub use hopper_data::HopperData;
pub use jigsaw::{EMPTY_LABEL, Joint};
pub use saved_item::{SavedItem, one};
pub use sign_text::SignText;
pub use spawn_data::SpawnData;
pub use text::Text;
pub use uuid::Uuid;
pub use vault_config::VaultConfig;
