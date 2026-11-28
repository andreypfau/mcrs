use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const MAP_DECORATION_TYPE: RegistryKey<crate::map_decoration_type_keys::MapDecorationType> =
    <crate::map_decoration_type_keys::MapDecorationType as Registered>::REGISTRY;
impl Registered for crate::map_decoration_type_keys::MapDecorationType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:map_decoration_type"));
}
