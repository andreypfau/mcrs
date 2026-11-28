use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CHUNK_STATUS: RegistryKey<crate::chunk_status_keys::ChunkStatus> =
    <crate::chunk_status_keys::ChunkStatus as Registered>::REGISTRY;
impl Registered for crate::chunk_status_keys::ChunkStatus {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:chunk_status"));
}
