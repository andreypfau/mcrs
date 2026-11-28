use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CHUNK_STATUS: RegistryKey<crate::chunk_status_keys::ChunkStatus> =
    RegistryKey::new(rl!("minecraft:chunk_status"));
impl Registered for crate::chunk_status_keys::ChunkStatus {
    const REGISTRY: RegistryKey<Self> = CHUNK_STATUS;
}
