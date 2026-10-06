// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod feature;
pub mod feature_tags;
pub mod placed_feature;
pub mod processor_list;
pub mod template_pool;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const FEATURE: RegistryKey<crate::proto::Feature> = RegistryKey::new(rl!("minecraft:worldgen/feature"));
impl Registered for crate::proto::Feature {
    const REGISTRY: RegistryKey<Self> = FEATURE;
}

pub const PLACED_FEATURE: RegistryKey<crate::proto::PlacedFeature> = RegistryKey::new(rl!("minecraft:worldgen/placed_feature"));
impl Registered for crate::proto::PlacedFeature {
    const REGISTRY: RegistryKey<Self> = PLACED_FEATURE;
}

pub const PROCESSOR_LIST: RegistryKey<crate::proto::StructureProcessorList> = RegistryKey::new(rl!("minecraft:worldgen/processor_list"));
impl Registered for crate::proto::StructureProcessorList {
    const REGISTRY: RegistryKey<Self> = PROCESSOR_LIST;
}

pub const TEMPLATE_POOL: RegistryKey<crate::pool::TemplatePool> = RegistryKey::new(rl!("minecraft:worldgen/template_pool"));
impl Registered for crate::pool::TemplatePool {
    const REGISTRY: RegistryKey<Self> = TEMPLATE_POOL;
}

pub fn bindings() -> [TypeBinding; 4] {
    [
        FEATURE.binding(),
        PLACED_FEATURE.binding(),
        PROCESSOR_LIST.binding(),
        TEMPLATE_POOL.binding(),
    ]
}
