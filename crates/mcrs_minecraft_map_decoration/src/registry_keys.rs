// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const MAP_DECORATION_TYPE: RegistryKey<crate::map_decoration_type_keys::MapDecorationType> =
    RegistryKey::new(rl!("minecraft:map_decoration_type"));
impl Registered for crate::map_decoration_type_keys::MapDecorationType {
    const REGISTRY: RegistryKey<Self> = MAP_DECORATION_TYPE;
}
