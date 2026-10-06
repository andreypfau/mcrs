// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod block;
pub mod block_entity_type;
pub mod block_tags;
pub mod fluid;
pub mod fluid_tags;

pub use block::Block;
pub use block_entity_type::BlockEntityType;
pub use fluid::Fluid;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const BLOCK: RegistryKey<crate::keys::Block> = RegistryKey::new(rl!("minecraft:block"));
impl Registered for crate::keys::Block {
    const REGISTRY: RegistryKey<Self> = BLOCK;
}

pub const BLOCK_ENTITY_TYPE: RegistryKey<crate::keys::BlockEntityType> = RegistryKey::new(rl!("minecraft:block_entity_type"));
impl Registered for crate::keys::BlockEntityType {
    const REGISTRY: RegistryKey<Self> = BLOCK_ENTITY_TYPE;
}

pub const FLUID: RegistryKey<crate::keys::Fluid> = RegistryKey::new(rl!("minecraft:fluid"));
impl Registered for crate::keys::Fluid {
    const REGISTRY: RegistryKey<Self> = FLUID;
}

pub fn bindings() -> [TypeBinding; 3] {
    [
        BLOCK.binding(),
        BLOCK_ENTITY_TYPE.binding(),
        FLUID.binding(),
    ]
}
