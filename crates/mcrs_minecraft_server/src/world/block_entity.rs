use std::borrow::Cow;
use std::io::Cursor;

use bevy_ecs::prelude::{Commands, Component};
use mcrs_minecraft_decoration::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::{Nbt, to_nbt_compound};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::chunk::ChunkDataBlockEntity;
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_world::world::dimension::InDimension;
use mcrs_voxel_world::world::storage::block_entity::BlockEntityPos;

/// A block entity's kind and state, in the one typed shape the save, the chunk
/// packet and the anvil reader share; the position is read off the entity's
/// [`BlockEntityPos`] and duplicated here only because the shape carries it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BlockEntity(pub GeneratedBlockEntity);

/// `BuiltInRegistries.BLOCK_ENTITY_TYPE` index of `minecraft:beehive`
/// (`world/level/block/entity/BlockEntityTypes.java`), which is how the chunk
/// packet names a block entity. The registry is built in rather than shipped,
/// so the order is the only place the number comes from.
const BEEHIVE_TYPE_ID: i32 = 33;
const CHEST_TYPE_ID: i32 = 1;
const MOB_SPAWNER_TYPE_ID: i32 = 9;
const END_GATEWAY_TYPE_ID: i32 = 22;

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

/// What the anvil reader hands back per chunk, read as the type itself.
pub fn from_compound(
    compound: &NbtCompound,
) -> Result<GeneratedBlockEntity, mcrs_minecraft_nbt::Error> {
    let bytes = Nbt::new(String::new(), compound.clone()).write_unnamed();
    mcrs_minecraft_nbt::from_bytes_unnamed(Cursor::new(bytes))
}

/// The chunk packet's entry, carrying the same compound the save holds.
pub fn packet_entry(
    entry: &GeneratedBlockEntity,
) -> Result<ChunkDataBlockEntity<'static>, mcrs_minecraft_nbt::Error> {
    let pos = entry.position();
    let kind = match entry {
        GeneratedBlockEntity::Beehive { .. } => BEEHIVE_TYPE_ID,
        GeneratedBlockEntity::Chest { .. } => CHEST_TYPE_ID,
        GeneratedBlockEntity::MobSpawner { .. } => MOB_SPAWNER_TYPE_ID,
        GeneratedBlockEntity::EndGateway(_) => END_GATEWAY_TYPE_ID,
    };
    Ok(ChunkDataBlockEntity {
        packed_xz: (((pos.x & 15) << 4) | (pos.z & 15)) as i8,
        y: pos.y as i16,
        kind: VarInt(kind),
        data: Cow::Owned(to_nbt_compound(entry)?),
    })
}
