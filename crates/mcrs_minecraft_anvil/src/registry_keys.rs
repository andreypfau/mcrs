use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const STRUCTURE_PIECE: RegistryKey<crate::structure_piece_keys::StructurePieceType> =
    <crate::structure_piece_keys::StructurePieceType as Registered>::REGISTRY;
impl Registered for crate::structure_piece_keys::StructurePieceType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/structure_piece"));
}

pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[(
    STRUCTURE_PIECE.location(),
    crate::structure_piece_keys::StructurePieceType::ENTRIES,
)];
