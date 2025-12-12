use crate::world::block::BlockUpdateFlags;
use crate::world::palette::BlockPalette;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Component, Query};
use bevy_ecs::query::{Changed, With, Without};
use mcrs_engine::entity::player::chunk_view::PlayerChunkObserver;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkIndex, ChunkPos};
use mcrs_network::ServerSideConnection;
use mcrs_protocol::chunk::ChunkBlockUpdateEntry;
use mcrs_protocol::packets::game::clientbound::ClientboundBlockUpdate;
use mcrs_protocol::{BlockStateId, ChunkColumnPos, Encode, Packet, WritePacket};
use rustc_hash::FxHashSet;
use std::borrow::Cow::Owned;

pub struct BlockUpdatePlugin;

impl Plugin for BlockUpdatePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_message::<BlockSetRequest>();
        app.add_message::<BlockPlaced>();
        app.add_systems(FixedUpdate, add_changes_set);
        app.add_systems(FixedUpdate, apply_set_block_request);
        app.add_systems(FixedPostUpdate, update_client_blocks);
    }
}

#[derive(Message)]
pub struct BlockSetRequest {
    pub dimension: Entity,
    pub pos: BlockPos,
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
    pub recursion_left: i16,
}

impl BlockSetRequest {
    pub fn remove_block<P: Into<BlockPos>>(dimension: Entity, pos: P) -> BlockSetRequest {
        BlockSetRequest {
            dimension,
            pos: pos.into(),
            new_state: BlockStateId(0),
            flags: BlockUpdateFlags::all(),
            recursion_left: 512,
        }
    }
}

trait SetBlock {
    fn set_block<P: Into<BlockPos>, S: Into<BlockStateId>>(
        &mut self,
        dimension: Entity,
        pos: P,
        new_state: S,
        flags: BlockUpdateFlags,
    );
}

impl<'s> SetBlock for MessageWriter<'s, BlockSetRequest> {
    fn set_block<P: Into<BlockPos>, S: Into<BlockStateId>>(
        &mut self,
        dimension: Entity,
        pos: P,
        new_state: S,
        flags: BlockUpdateFlags,
    ) {
        self.write(BlockSetRequest {
            dimension,
            pos: pos.into(),
            new_state: new_state.into(),
            flags,
            recursion_left: 10,
        });
    }
}

#[derive(Default, Component)]
struct ChunkNetworkSyncBlockChangesSet {
    pub changes: FxHashSet<BlockPos>,
}

fn add_changes_set(
    query: Query<Entity, (With<BlockPalette>, Without<ChunkNetworkSyncBlockChangesSet>)>,
    mut commands: Commands,
) {
    for entity in query.iter() {
        commands
            .entity(entity)
            .insert(ChunkNetworkSyncBlockChangesSet::default());
    }
}

fn apply_set_block_request(
    mut reader: MessageReader<BlockSetRequest>,
    dimensions: Query<&ChunkIndex>,
    mut chunks: Query<(
        Entity,
        &mut BlockPalette,
        &mut ChunkNetworkSyncBlockChangesSet,
    )>,
    mut writer: MessageWriter<BlockPlaced>,
) {
    reader.read().for_each(|request| {
        let chunk_pos = ChunkPos::from(request.pos);

        let Ok(chunk_index) = dimensions.get(request.dimension) else {
            return;
        };
        let Some((chunk, mut storage, mut changes)) = chunk_index
            .get(chunk_pos)
            .and_then(|e| chunks.get_mut(e).ok())
        else {
            return;
        };

        let old_state = storage.set(request.pos, request.new_state);
        if old_state == request.new_state {
            return;
        }
        if request.flags.contains(BlockUpdateFlags::CLIENTS) {
            changes.changes.insert(request.pos);
        }

        writer.write(BlockPlaced {
            chunk,
            chunk_pos,
            block_pos: request.pos,
            old_state,
            new_state: request.new_state,
            flags: request.flags,
        });
    });
}

#[derive(Message)]
pub struct BlockPlaced {
    chunk: Entity,
    chunk_pos: ChunkPos,
    block_pos: BlockPos,
    old_state: BlockStateId,
    new_state: BlockStateId,
    flags: BlockUpdateFlags,
}

fn update_client_blocks(
    mut chunks: Query<
        (
            &ChunkPos,
            &mut BlockPalette,
            &mut ChunkNetworkSyncBlockChangesSet,
        ),
        Changed<ChunkNetworkSyncBlockChangesSet>,
    >,
    mut players: Query<(&PlayerChunkObserver, &mut ServerSideConnection)>,
) {
    fn flush_packet<P>(
        players: &mut Query<(&PlayerChunkObserver, &mut ServerSideConnection)>,
        packet: &P,
        column: &ChunkColumnPos,
    ) where
        P: Packet + Encode + Sync,
    {
        players.par_iter_mut().for_each(|(observer, mut con)| {
            if observer
                .last_last_chunk_tracking_view
                .map(|v| v.contains(&ChunkPos::new(column.x, v.center.y, column.z)))
                .unwrap_or(false)
            {
                con.write_packet(packet);
            }
        });
    }

    chunks
        .iter_mut()
        .for_each(|(chunk_pos, storage, mut changes)| {
            let chunk_column_pos = ChunkColumnPos::from(*chunk_pos);
            if changes.changes.len() <= 1 {
                changes.changes.retain(|pos| {
                    let pkt = ClientboundBlockUpdate {
                        block_pos: *pos,
                        block_state_id: storage.get(*pos),
                    };
                    flush_packet(&mut players, &pkt, &chunk_column_pos);
                    false
                });
            } else {
                let mut updates = Vec::with_capacity(changes.changes.len());
                changes.changes.retain(|pos| {
                    let entry = ChunkBlockUpdateEntry::new()
                        .with_off_x((pos.x & 0x0F) as u8)
                        .with_off_y((pos.y & 0x0F) as u8)
                        .with_off_z((pos.z & 0x0F) as u8)
                        .with_block_state(storage.get(*pos).0);
                    updates.push(entry);
                    false
                });
                let pkt =
                    mcrs_protocol::packets::game::clientbound::ClientboundSectionBlocksUpdate {
                        chunk_pos: *chunk_pos,
                        blocks: Owned(updates),
                    };
                flush_packet(&mut players, &pkt, &chunk_column_pos);
            }
        });
}
