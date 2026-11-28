use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const FLOAT_PROVIDER_TYPE: RegistryKey<crate::float_provider_type_keys::FloatProviderType> =
    <crate::float_provider_type_keys::FloatProviderType as Registered>::REGISTRY;
impl Registered for crate::float_provider_type_keys::FloatProviderType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:float_provider_type"));
}

pub const HEIGHT_PROVIDER_TYPE: RegistryKey<crate::height_provider_type_keys::HeightProviderType> =
    <crate::height_provider_type_keys::HeightProviderType as Registered>::REGISTRY;
impl Registered for crate::height_provider_type_keys::HeightProviderType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:height_provider_type"));
}

pub const INT_PROVIDER_TYPE: RegistryKey<crate::int_provider_type_keys::IntProviderType> =
    <crate::int_provider_type_keys::IntProviderType as Registered>::REGISTRY;
impl Registered for crate::int_provider_type_keys::IntProviderType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:int_provider_type"));
}
