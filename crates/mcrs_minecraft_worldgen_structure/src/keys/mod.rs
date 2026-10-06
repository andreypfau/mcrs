// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod pool_alias_binding;
pub mod spawn_condition_type;
pub mod structure;
pub mod structure_piece;
pub mod structure_placement;
pub mod structure_set;
pub mod structure_tags;
pub mod structure_type;

pub use spawn_condition_type::SpawnConditionType;
pub use pool_alias_binding::PoolAliasBindingType;
pub use structure_piece::StructurePieceType;
pub use structure_placement::StructurePlacementType;
pub use structure_type::StructureType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const SPAWN_CONDITION_TYPE: RegistryKey<crate::keys::SpawnConditionType> = RegistryKey::new(rl!("minecraft:spawn_condition_type"));
impl Registered for crate::keys::SpawnConditionType {
    const REGISTRY: RegistryKey<Self> = SPAWN_CONDITION_TYPE;
}

pub const POOL_ALIAS_BINDING: RegistryKey<crate::keys::PoolAliasBindingType> = RegistryKey::new(rl!("minecraft:worldgen/pool_alias_binding"));
impl Registered for crate::keys::PoolAliasBindingType {
    const REGISTRY: RegistryKey<Self> = POOL_ALIAS_BINDING;
}

pub const STRUCTURE: RegistryKey<crate::Structure> = RegistryKey::new(rl!("minecraft:worldgen/structure"));
impl Registered for crate::Structure {
    const REGISTRY: RegistryKey<Self> = STRUCTURE;
}

pub const STRUCTURE_PIECE: RegistryKey<crate::keys::StructurePieceType> = RegistryKey::new(rl!("minecraft:worldgen/structure_piece"));
impl Registered for crate::keys::StructurePieceType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PIECE;
}

pub const STRUCTURE_PLACEMENT: RegistryKey<crate::keys::StructurePlacementType> = RegistryKey::new(rl!("minecraft:worldgen/structure_placement"));
impl Registered for crate::keys::StructurePlacementType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PLACEMENT;
}

pub const STRUCTURE_SET: RegistryKey<crate::StructureSet> = RegistryKey::new(rl!("minecraft:worldgen/structure_set"));
impl Registered for crate::StructureSet {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_SET;
}

pub const STRUCTURE_TYPE: RegistryKey<crate::keys::StructureType> = RegistryKey::new(rl!("minecraft:worldgen/structure_type"));
impl Registered for crate::keys::StructureType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_TYPE;
}

pub fn bindings() -> [TypeBinding; 7] {
    [
        SPAWN_CONDITION_TYPE.binding(),
        POOL_ALIAS_BINDING.binding(),
        STRUCTURE.binding(),
        STRUCTURE_PIECE.binding(),
        STRUCTURE_PLACEMENT.binding(),
        STRUCTURE_SET.binding(),
        STRUCTURE_TYPE.binding(),
    ]
}
