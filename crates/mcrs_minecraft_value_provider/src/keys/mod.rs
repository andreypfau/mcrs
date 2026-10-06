// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod float_provider_type;
pub mod height_provider_type;
pub mod int_provider_type;

pub use float_provider_type::FloatProviderType;
pub use height_provider_type::HeightProviderType;
pub use int_provider_type::IntProviderType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const FLOAT_PROVIDER_TYPE: RegistryKey<crate::keys::FloatProviderType> = RegistryKey::new(rl!("minecraft:float_provider_type"));
impl Registered for crate::keys::FloatProviderType {
    const REGISTRY: RegistryKey<Self> = FLOAT_PROVIDER_TYPE;
}

pub const HEIGHT_PROVIDER_TYPE: RegistryKey<crate::keys::HeightProviderType> = RegistryKey::new(rl!("minecraft:height_provider_type"));
impl Registered for crate::keys::HeightProviderType {
    const REGISTRY: RegistryKey<Self> = HEIGHT_PROVIDER_TYPE;
}

pub const INT_PROVIDER_TYPE: RegistryKey<crate::keys::IntProviderType> = RegistryKey::new(rl!("minecraft:int_provider_type"));
impl Registered for crate::keys::IntProviderType {
    const REGISTRY: RegistryKey<Self> = INT_PROVIDER_TYPE;
}

pub fn bindings() -> [TypeBinding; 3] {
    [
        FLOAT_PROVIDER_TYPE.binding(),
        HEIGHT_PROVIDER_TYPE.binding(),
        INT_PROVIDER_TYPE.binding(),
    ]
}
