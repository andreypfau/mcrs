// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ATTRIBUTE: RegistryKey<crate::attribute_keys::Attribute> =
    RegistryKey::new(rl!("minecraft:attribute"));
impl Registered for crate::attribute_keys::Attribute {
    const REGISTRY: RegistryKey<Self> = ATTRIBUTE;
}
