// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const MENU: RegistryKey<crate::menu_keys::MenuType> = RegistryKey::new(rl!("minecraft:menu"));
impl Registered for crate::menu_keys::MenuType {
    const REGISTRY: RegistryKey<Self> = MENU;
}
