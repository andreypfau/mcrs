use crate::block::BlockUpdateFlags;
use crate::palette::BlockPalette;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Component, Query};
use bevy_ecs::query::{With, Without};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkIndex, ChunkPos};
use mcrs_protocol::BlockStateId;
use rustc_hash::FxHashSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockUpdateSet {
    ApplyChanges,
    NetworkSync,
}

pub struct BlockUpdatePlugin;

impl Plugin for BlockUpdatePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_message::<BlockSetRequest>();
        app.add_message::<BlockPlaced>();
        app.configure_sets(FixedUpdate, BlockUpdateSet::ApplyChanges);
        app.configure_sets(FixedPostUpdate, BlockUpdateSet::NetworkSync);
        app.add_systems(FixedUpdate, add_changes_set);
        app.add_systems(
            FixedUpdate,
            apply_set_block_request.in_set(BlockUpdateSet::ApplyChanges),
        );
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
pub struct ChunkNetworkSyncBlockChangesSet {
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

pub fn apply_set_block_request(
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
            // Stale dimension Entity. Treated as suspicious because the
            // dimension typically outlives a single FixedUpdate; the
            // most common cause is a bus race between dim despawn and
            // BlockSetRequest delivery.
            tracing::warn!(
                target: "block_update",
                dimension = ?request.dimension,
                pos = ?request.pos,
                "apply_set_block_request: dimension lookup failed; dropping",
            );
            return;
        };
        let Some((chunk, mut storage, mut changes)) = chunk_index
            .get(chunk_pos)
            .and_then(|e| chunks.get_mut(e).ok())
        else {
            // Chunk unloaded or its palette query missed. This is
            // expected (a block update can arrive for a chunk that just
            // unloaded) and not an error.
            tracing::trace!(
                target: "block_update",
                ?chunk_pos,
                pos = ?request.pos,
                "apply_set_block_request: chunk not present in ChunkIndex; dropping",
            );
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

#[derive(Message, Clone, Copy)]
pub struct BlockPlaced {
    pub chunk: Entity,
    pub chunk_pos: ChunkPos,
    pub block_pos: BlockPos,
    pub old_state: BlockStateId,
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_configured_compile_test() {
        let _ = apply_set_block_request.in_set(BlockUpdateSet::ApplyChanges);
    }

    #[test]
    fn block_placed_fields_pub_compile_test() {
        let _ = BlockPlaced {
            chunk: Entity::PLACEHOLDER,
            chunk_pos: ChunkPos::new(0, 0, 0),
            block_pos: BlockPos::new(0, 0, 0),
            old_state: BlockStateId(0),
            new_state: BlockStateId(0),
            flags: BlockUpdateFlags::all(),
        };
    }
}
