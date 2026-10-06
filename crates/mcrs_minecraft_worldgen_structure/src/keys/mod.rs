// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod structure;
pub mod structure_set;
pub mod structure_tags;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const STRUCTURE: RegistryKey<crate::Structure> = RegistryKey::new(rl!("minecraft:worldgen/structure"));
impl Registered for crate::Structure {
    const REGISTRY: RegistryKey<Self> = STRUCTURE;
}

pub const STRUCTURE_SET: RegistryKey<crate::StructureSet> = RegistryKey::new(rl!("minecraft:worldgen/structure_set"));
impl Registered for crate::StructureSet {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_SET;
}

pub fn bindings() -> [TypeBinding; 2] {
    [
        STRUCTURE.binding(),
        STRUCTURE_SET.binding(),
    ]
}
