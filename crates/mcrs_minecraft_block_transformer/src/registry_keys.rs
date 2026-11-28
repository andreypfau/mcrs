use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK_TRANSFORMER: RegistryKey<crate::BlockTransformer> =
    <crate::BlockTransformer as Registered>::REGISTRY;
impl Registered for crate::BlockTransformer {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:block_transformer"));
}
