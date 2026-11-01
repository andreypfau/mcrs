// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CHUNK_STATUS: RegistryKey<crate::chunk_status_keys::ChunkStatus> =
    RegistryKey::new(rl!("minecraft:chunk_status"));
impl Registered for crate::chunk_status_keys::ChunkStatus {
    const REGISTRY: RegistryKey<Self> = CHUNK_STATUS;
}
