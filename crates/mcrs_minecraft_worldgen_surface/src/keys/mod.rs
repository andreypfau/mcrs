// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod material_rule;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const MATERIAL_RULE: RegistryKey<crate::proto::MaterialRule> = RegistryKey::new(rl!("minecraft:worldgen/material_rule"));
impl Registered for crate::proto::MaterialRule {
    const REGISTRY: RegistryKey<Self> = MATERIAL_RULE;
}

pub fn bindings() -> [TypeBinding; 1] {
    [
        MATERIAL_RULE.binding(),
    ]
}
