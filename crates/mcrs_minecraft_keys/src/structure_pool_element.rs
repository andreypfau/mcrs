// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const EMPTY_POOL_ELEMENT: Id<crate::StructurePoolElement> = Id::from_static(3);
pub const FEATURE_POOL_ELEMENT: Id<crate::StructurePoolElement> = Id::from_static(2);
pub const LEGACY_SINGLE_POOL_ELEMENT: Id<crate::StructurePoolElement> = Id::from_static(4);
pub const LIST_POOL_ELEMENT: Id<crate::StructurePoolElement> = Id::from_static(1);
pub const SINGLE_POOL_ELEMENT: Id<crate::StructurePoolElement> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:single_pool_element",
    "minecraft:list_pool_element",
    "minecraft:feature_pool_element",
    "minecraft:empty_pool_element",
    "minecraft:legacy_single_pool_element",
];
