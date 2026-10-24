// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod chunk_status;

pub use chunk_status::ChunkStatus;

use mcrs_minecraft_core::{Registered, RegistryKey, rl};

pub const CHUNK_STATUS: RegistryKey<crate::keys::ChunkStatus> =
    RegistryKey::new(rl!("minecraft:chunk_status"));
impl Registered for crate::keys::ChunkStatus {
    const REGISTRY: RegistryKey<Self> = CHUNK_STATUS;
}
