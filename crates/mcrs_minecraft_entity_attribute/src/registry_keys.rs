use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ATTRIBUTE: RegistryKey<crate::attribute_keys::Attribute> =
    <crate::attribute_keys::Attribute as Registered>::REGISTRY;
impl Registered for crate::attribute_keys::Attribute {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:attribute"));
}
