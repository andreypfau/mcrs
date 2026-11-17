use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const CARVER_TYPE: RegistryKey<crate::carver_type_keys::CarverType> =
    RegistryKey::new(rl!("minecraft:worldgen/carver_type"));
impl Registered for crate::carver_type_keys::CarverType {
    const REGISTRY: RegistryKey<Self> = CARVER_TYPE;
}

pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[(
    CARVER_TYPE.location(),
    crate::carver_type_keys::CarverType::ENTRIES,
)];
