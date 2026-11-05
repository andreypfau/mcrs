//! The block events that the blocks of a dimension took, for the clients that hold their
//! columns.

use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockEvent;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::BlockEvent;
use mcrs_voxel::ColumnPos;

use crate::bus::{PacketPayload, PacketPriority};
use crate::column_change::ColumnChange;
use crate::column_streams::ColumnStreams;

/// Publishes each event of `delivered` in a column that the muxer subscribes to. The muxer sends
/// it to the clients that hold the column, as the owner of the column publishes its changes.
pub(crate) fn publish_block_events(
    delivered: &[BlockEvent<Id<Block>>],
    streams: &ColumnStreams,
    changes: &mut MessageWriter<ColumnChange>,
) {
    for event in delivered {
        let column = ColumnPos::from(event.pos);
        if !streams.is_subscribed(column) {
            continue;
        }
        changes.write(ColumnChange {
            column,
            priority: PacketPriority::Normal,
            // The client takes only the low 8 bits of each argument.
            payload: PacketPayload::BlockEvent(ClientboundBlockEvent {
                pos: event.pos,
                action: event.a as u8,
                param: event.b as u8,
                block: VarInt(i32::from(event.block.number())),
            }),
        });
    }
}
