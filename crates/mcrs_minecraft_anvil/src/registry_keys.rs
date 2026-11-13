// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const STRUCTURE_PIECE: RegistryKey<crate::structure_piece_keys::StructurePieceType> =
    RegistryKey::new(rl!("minecraft:worldgen/structure_piece"));
impl Registered for crate::structure_piece_keys::StructurePieceType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PIECE;
}

/// The static registries of this crate, which the catalog does not list.
pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[(
    STRUCTURE_PIECE.location(),
    crate::structure_piece_keys::StructurePieceType::ENTRIES,
)];
