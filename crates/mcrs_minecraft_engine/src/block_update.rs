//! The block changes that a dimension publishes for the clients: one change of each block that
//! a tick placed in a column that the muxer subscribes to.

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Entity, Local, Query};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockUpdate;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::BlockPos;
use mcrs_voxel::ColumnPos;
use mcrs_voxel::LocalPos;
use rustc_hash::FxHashMap;

use crate::bus::{PacketPayload, PacketPriority};
use crate::column_change::ColumnChange;
use crate::column_streams::ColumnStreams;

/// Publishes the blocks placed this tick that notify clients, in a column that the muxer
/// subscribes to. A block placed more than once in a tick goes out once, with the state it
/// holds when the change is built.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "block_update::publish_block_changes", skip_all)
)]
pub fn publish_block_changes(
    mut placed: MessageReader<BlockPlaced>,
    sections: Query<(&InDimension, &ChunkBlocks)>,
    streams: Query<&ColumnStreams>,
    mut changes: MessageWriter<ColumnChange>,
    mut changed: Local<FxHashMap<BlockPos, Entity>>,
) {
    for placed in placed.read() {
        if placed.flags.contains(BlockUpdateFlags::CLIENTS) {
            changed.insert(placed.block_pos, placed.chunk);
        }
    }

    for (position, section) in changed.drain() {
        let Ok((in_dim, palette)) = sections.get(section) else {
            continue;
        };
        let column = ColumnPos::from(position);
        if !streams
            .get(in_dim.0)
            .is_ok_and(|streams| streams.is_subscribed(column))
        {
            continue;
        }
        let new_state = BlockStateNumber(palette.get(LocalPos::from(position)).number());
        changes.write(ColumnChange {
            column,
            priority: PacketPriority::Normal,
            payload: PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                block_pos: position,
                block_state_id: new_state,
            }),
        });
    }
}
