// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK_TRANSFORMER: RegistryKey<crate::BlockTransformer> =
    RegistryKey::new(rl!("minecraft:block_transformer"));
impl Registered for crate::BlockTransformer {
    const REGISTRY: RegistryKey<Self> = BLOCK_TRANSFORMER;
}
