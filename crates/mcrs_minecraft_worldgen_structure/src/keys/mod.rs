// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod pool_alias_binding;
pub mod structure_piece;
pub mod structure_placement;
pub mod structure_set;
pub mod structure_type;

pub use pool_alias_binding::PoolAliasBindingType;
pub use structure_piece::StructurePieceType;
pub use structure_placement::StructurePlacementType;
pub use structure_type::StructureType;

use mcrs_minecraft_core::{Registered, RegistryKey, StaticResourceLocation, rl};

pub const POOL_ALIAS_BINDING: RegistryKey<crate::keys::PoolAliasBindingType> =
    RegistryKey::new(rl!("minecraft:worldgen/pool_alias_binding"));
impl Registered for crate::keys::PoolAliasBindingType {
    const REGISTRY: RegistryKey<Self> = POOL_ALIAS_BINDING;
}

pub const STRUCTURE_PIECE: RegistryKey<crate::keys::StructurePieceType> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_piece"));
impl Registered for crate::keys::StructurePieceType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PIECE;
}

pub const STRUCTURE_PLACEMENT: RegistryKey<crate::keys::StructurePlacementType> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_placement"));
impl Registered for crate::keys::StructurePlacementType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PLACEMENT;
}

pub const STRUCTURE_SET: RegistryKey<crate::StructureSet> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_set"));
impl Registered for crate::StructureSet {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_SET;
}

pub const STRUCTURE_TYPE: RegistryKey<crate::keys::StructureType> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_type"));
impl Registered for crate::keys::StructureType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_TYPE;
}

/// The static registries of this crate, which the catalog does not list.
pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[
    (
        STRUCTURE_PLACEMENT.location(),
        StructurePlacementType::ENTRIES,
    ),
    (STRUCTURE_PIECE.location(), StructurePieceType::ENTRIES),
    (STRUCTURE_TYPE.location(), StructureType::ENTRIES),
    (POOL_ALIAS_BINDING.location(), PoolAliasBindingType::ENTRIES),
];
