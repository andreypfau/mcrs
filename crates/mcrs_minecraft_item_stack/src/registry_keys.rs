use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const DATA_COMPONENT_TYPE: RegistryKey<crate::data_component_type_keys::DataComponentType> =
    RegistryKey::new(rl!("minecraft:data_component_type"));
impl Registered for crate::data_component_type_keys::DataComponentType {
    const REGISTRY: RegistryKey<Self> = DATA_COMPONENT_TYPE;
}
