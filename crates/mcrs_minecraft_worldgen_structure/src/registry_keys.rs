use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const POOL_ALIAS_BINDING: RegistryKey<crate::pool_alias_binding_keys::PoolAliasBindingType> =
    <crate::pool_alias_binding_keys::PoolAliasBindingType as Registered>::REGISTRY;
impl Registered for crate::pool_alias_binding_keys::PoolAliasBindingType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/pool_alias_binding"));
}

pub const STRUCTURE_PLACEMENT: RegistryKey<
    crate::structure_placement_keys::StructurePlacementType,
> = <crate::structure_placement_keys::StructurePlacementType as Registered>::REGISTRY;
impl Registered for crate::structure_placement_keys::StructurePlacementType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/structure_placement"));
}

pub const STRUCTURE_SET: RegistryKey<crate::StructureSet> =
    <crate::StructureSet as Registered>::REGISTRY;
impl Registered for crate::StructureSet {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/structure_set"));
}

pub const STRUCTURE_TYPE: RegistryKey<crate::structure_type_keys::StructureType> =
    <crate::structure_type_keys::StructureType as Registered>::REGISTRY;
impl Registered for crate::structure_type_keys::StructureType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/structure_type"));
}

pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[
    (
        STRUCTURE_PLACEMENT.location(),
        crate::structure_placement_keys::StructurePlacementType::ENTRIES,
    ),
    (
        STRUCTURE_TYPE.location(),
        crate::structure_type_keys::StructureType::ENTRIES,
    ),
    (
        POOL_ALIAS_BINDING.location(),
        crate::pool_alias_binding_keys::PoolAliasBindingType::ENTRIES,
    ),
];
