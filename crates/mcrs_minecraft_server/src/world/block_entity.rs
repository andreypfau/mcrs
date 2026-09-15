use bevy_ecs::prelude::{Commands, Component};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::block_entity::BlockEntityPos;
use mcrs_minecraft_nbt::to_nbt_compound;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::chunk::ChunkDataBlockEntity;
use mcrs_minecraft_worldgen_feature_place::block_entity::BLOCK_ENTITY_TYPES;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use std::borrow::Cow;

/// A block entity's kind and state, in the one typed shape the save, the chunk
/// packet and the anvil reader share; the position is read off the entity's
/// [`BlockEntityPos`] and duplicated here only because the shape carries it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BlockEntity(pub GeneratedBlockEntity);

/// One entity per entry, under the dimension whose sections the reconcile step
/// resolves them against.
pub fn spawn_block_entities(
    commands: &mut Commands,
    dim: InDimension,
    entries: Vec<GeneratedBlockEntity>,
) {
    for entry in entries {
        let pos = entry.position();
        commands.spawn((
            BlockEntityPos(BlockPos::new(pos.x, pos.y, pos.z)),
            dim,
            BlockEntity(entry),
        ));
    }
}

/// The chunk packet's entry, carrying the same compound the save holds.
pub fn packet_entry(
    entry: &GeneratedBlockEntity,
) -> Result<ChunkDataBlockEntity<'static>, mcrs_minecraft_nbt::Error> {
    let pos = entry.position();
    let data = to_nbt_compound(entry)?;
    let id = data.get_string("id").expect("the enum is tagged by id");
    let kind = BLOCK_ENTITY_TYPES
        .iter()
        .position(|kind| *kind == id)
        .expect("every modelled block entity is a registered type") as i32;
    Ok(ChunkDataBlockEntity {
        packed_xz: (((pos.x & 15) << 4) | (pos.z & 15)) as i8,
        y: pos.y as i16,
        kind: VarInt(kind),
        data: Cow::Owned(data),
    })
}
