// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ITEM: RegistryKey<crate::item_keys::Item> = RegistryKey::new(rl!("minecraft:item"));
impl Registered for crate::item_keys::Item {
    const REGISTRY: RegistryKey<Self> = ITEM;
}
