// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const FLOAT_PROVIDER_TYPE: RegistryKey<crate::float_provider_type_keys::FloatProviderType> =
    RegistryKey::new(rl!("minecraft:float_provider_type"));
impl Registered for crate::float_provider_type_keys::FloatProviderType {
    const REGISTRY: RegistryKey<Self> = FLOAT_PROVIDER_TYPE;
}

pub const HEIGHT_PROVIDER_TYPE: RegistryKey<crate::height_provider_type_keys::HeightProviderType> =
    RegistryKey::new(rl!("minecraft:height_provider_type"));
impl Registered for crate::height_provider_type_keys::HeightProviderType {
    const REGISTRY: RegistryKey<Self> = HEIGHT_PROVIDER_TYPE;
}

pub const INT_PROVIDER_TYPE: RegistryKey<crate::int_provider_type_keys::IntProviderType> =
    RegistryKey::new(rl!("minecraft:int_provider_type"));
impl Registered for crate::int_provider_type_keys::IntProviderType {
    const REGISTRY: RegistryKey<Self> = INT_PROVIDER_TYPE;
}
