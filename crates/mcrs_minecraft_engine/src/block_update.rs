//! The block changes that a dimension publishes for the clients: one change of each section in
//! which a tick placed blocks, in a column that the muxer subscribes to.

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Entity, Local, Query, Res};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntityPos, SectionBlockEntities,
};
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::chunk::ChunkBlockUpdateEntry;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockUpdate, ClientboundSectionBlocksUpdate,
};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::BlockPos;
use mcrs_voxel::ColumnPos;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::bus::{PacketPayload, PacketPriority};
use crate::column_change::ColumnChange;
use crate::column_streams::ColumnStreams;

/// Publishes the blocks placed this tick that notify clients, in a column that the muxer
/// subscribes to: a `block_update` for a section with one changed block, a
/// `section_blocks_update` for a section with several. A block placed more than once in a tick
/// goes out once, with the state it holds when the change is built.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "block_update::publish_block_changes", skip_all)
)]
pub fn publish_block_changes(
    mut placed: MessageReader<BlockPlaced>,
    sections: Query<(&InDimension, &ChunkBlocks)>,
    streams: Query<&ColumnStreams>,
    mut changes: MessageWriter<ColumnChange>,
    mut changed: Local<FxHashMap<Entity, (SectionPos, FxHashSet<BlockPos>)>>,
) {
    for placed in placed.read() {
        if placed.flags.contains(BlockUpdateFlags::CLIENTS) {
            changed
                .entry(placed.chunk)
                .or_insert_with(|| (placed.chunk_pos, FxHashSet::default()))
                .1
                .insert(placed.block_pos);
        }
    }

    for (section, (section_pos, positions)) in changed.drain() {
        let Ok((in_dim, palette)) = sections.get(section) else {
            continue;
        };
        let column = ColumnPos::from(section_pos);
        if !streams
            .get(in_dim.0)
            .is_ok_and(|streams| streams.is_subscribed(column))
        {
            continue;
        }
        let state = |position: BlockPos| palette.get(LocalPos::from(position)).number();
        let payload = match positions.iter().next() {
            Some(&position) if positions.len() == 1 => {
                PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                    block_pos: position,
                    block_state_id: BlockStateNumber(state(position)),
                })
            }
            _ => PacketPayload::SectionBlocksUpdate(ClientboundSectionBlocksUpdate {
                chunk_pos: section_pos,
                blocks: positions
                    .iter()
                    .map(|&position| {
                        let local = LocalPos::from(position);
                        ChunkBlockUpdateEntry::new()
                            .with_off_x(local.x())
                            .with_off_y(local.y())
                            .with_off_z(local.z())
                            .with_block_state(state(position))
                    })
                    .collect::<Vec<_>>()
                    .into(),
            }),
        };
        changes.write(ColumnChange {
            column,
            priority: PacketPriority::Normal,
            payload,
            except: None,
        });
    }
}

/// Removes the block entity at each position that a block of another type took this tick, so
/// the column no longer carries it. A new state of the same block keeps its block entity.
pub fn remove_replaced_block_entities(
    mut placed: MessageReader<BlockPlaced>,
    sections: Query<&SectionBlockEntities>,
    positions: Query<&BlockEntityPos>,
    blocks: Option<Res<Blocks>>,
    mut commands: Commands,
) {
    let Some(blocks) = blocks else {
        placed.clear();
        return;
    };
    for placed in placed.read() {
        if blocks.block_index(placed.old_state) == blocks.block_index(placed.new_state) {
            continue;
        }
        let Ok(held) = sections.get(placed.chunk) else {
            continue;
        };
        for &entity in held.iter() {
            if positions
                .get(entity)
                .is_ok_and(|at| at.0 == placed.block_pos)
            {
                commands.entity(entity).despawn();
            }
        }
    }
}
