// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const DENSITY_FUNCTION: RegistryKey<crate::proto::DensityFunctionHolder> =
    RegistryKey::new(rl!("minecraft:worldgen/density_function"));
impl Registered for crate::proto::DensityFunctionHolder {
    const REGISTRY: RegistryKey<Self> = DENSITY_FUNCTION;
}

pub const DENSITY_FUNCTION_TYPE: RegistryKey<
    crate::density_function_type_keys::DensityFunctionType,
> = RegistryKey::new(rl!("minecraft:worldgen/density_function_type"));
impl Registered for crate::density_function_type_keys::DensityFunctionType {
    const REGISTRY: RegistryKey<Self> = DENSITY_FUNCTION_TYPE;
}

/// The static registries of this crate, which the catalog does not list.
pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[(
    DENSITY_FUNCTION_TYPE.location(),
    crate::density_function_type_keys::DensityFunctionType::ENTRIES,
)];
