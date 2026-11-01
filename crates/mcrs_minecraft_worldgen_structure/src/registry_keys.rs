// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const POOL_ALIAS_BINDING: RegistryKey<crate::pool_alias_binding_keys::PoolAliasBindingType> =
    RegistryKey::new(rl!("minecraft:worldgen/pool_alias_binding"));
impl Registered for crate::pool_alias_binding_keys::PoolAliasBindingType {
    const REGISTRY: RegistryKey<Self> = POOL_ALIAS_BINDING;
}

pub const STRUCTURE_PIECE: RegistryKey<crate::structure_piece_keys::StructurePieceType> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_piece"));
impl Registered for crate::structure_piece_keys::StructurePieceType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PIECE;
}

pub const STRUCTURE_PLACEMENT: RegistryKey<
    crate::structure_placement_keys::StructurePlacementType,
> = RegistryKey::new(rl!("minecraft:worldgen/structure_placement"));
impl Registered for crate::structure_placement_keys::StructurePlacementType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PLACEMENT;
}

pub const STRUCTURE_SET: RegistryKey<crate::StructureSet> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_set"));
impl Registered for crate::StructureSet {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_SET;
}

pub const STRUCTURE_TYPE: RegistryKey<crate::structure_type_keys::StructureType> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_type"));
impl Registered for crate::structure_type_keys::StructureType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_TYPE;
}

/// The static registries of this crate, which the catalog does not list.
pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[
    (
        STRUCTURE_PLACEMENT.location(),
        crate::structure_placement_keys::StructurePlacementType::ENTRIES,
    ),
    (
        STRUCTURE_PIECE.location(),
        crate::structure_piece_keys::StructurePieceType::ENTRIES,
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
