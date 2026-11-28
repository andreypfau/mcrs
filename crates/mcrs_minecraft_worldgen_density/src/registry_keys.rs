use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const DENSITY_FUNCTION: RegistryKey<crate::proto::DensityFunctionHolder> =
    <crate::proto::DensityFunctionHolder as Registered>::REGISTRY;
impl Registered for crate::proto::DensityFunctionHolder {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/density_function"));
}

pub const DENSITY_FUNCTION_TYPE: RegistryKey<
    crate::density_function_type_keys::DensityFunctionType,
> = <crate::density_function_type_keys::DensityFunctionType as Registered>::REGISTRY;
impl Registered for crate::density_function_type_keys::DensityFunctionType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/density_function_type"));
}

pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[(
    DENSITY_FUNCTION_TYPE.location(),
    crate::density_function_type_keys::DensityFunctionType::ENTRIES,
)];
