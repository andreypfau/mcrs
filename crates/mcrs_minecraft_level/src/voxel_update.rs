use crate::block::BlockUpdateFlags;
use crate::palette::ChunkBlocks;
use crate::world::storage::section::SectionIndex;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter, Messages};
use bevy_ecs::prelude::Query;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::{LocalPos, SectionPos};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VoxelUpdateSet {
    ApplyChanges,
    NetworkSync,
}

#[derive(Default)]
pub struct BlockUpdatePlugin;

impl Plugin for BlockUpdatePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        // Messages<BlockSetRequest> and Messages<BlockPlaced> are
        // registered by the caller (the per-dim sub-app builder) before
        // this plugin runs. Registering them here would re-export the
        // buffers if the plugin were ever added host-side by mistake,
        // hiding the per-dim invariant.
        debug_assert!(
            app.world().contains_resource::<Messages<BlockSetRequest>>(),
            "BlockUpdatePlugin requires Messages<BlockSetRequest> registered by the per-dim sub-app builder before add_plugins",
        );
        debug_assert!(
            app.world().contains_resource::<Messages<BlockPlaced>>(),
            "BlockUpdatePlugin requires Messages<BlockPlaced> registered by the per-dim sub-app builder before add_plugins",
        );
        app.configure_sets(FixedUpdate, VoxelUpdateSet::ApplyChanges);
        app.configure_sets(FixedPostUpdate, VoxelUpdateSet::NetworkSync);
        app.add_systems(
            FixedUpdate,
            apply_voxel_set_requests.in_set(VoxelUpdateSet::ApplyChanges),
        );
    }
}

#[derive(Message)]
pub struct BlockSetRequest {
    pub dimension: Entity,
    pub pos: BlockPos,
    pub new_state: VoxelId,
    pub flags: BlockUpdateFlags,
    pub recursion_left: i16,
}

pub fn apply_voxel_set_requests(
    mut reader: MessageReader<BlockSetRequest>,
    dimensions: Query<&SectionIndex>,
    mut chunks: Query<(Entity, &mut ChunkBlocks)>,
    mut writer: MessageWriter<BlockPlaced>,
) {
    reader.read().for_each(|request| {
        let chunk_pos = SectionPos::from(request.pos);

        let Ok(chunk_index) = dimensions.get(request.dimension) else {
            // Stale dimension Entity. Treated as suspicious because the
            // dimension typically outlives a single FixedUpdate; the
            // most common cause is a bus race between dim despawn and
            // BlockSetRequest delivery.
            tracing::warn!(
                target: "voxel_update",
                dimension = ?request.dimension,
                pos = ?request.pos,
                "apply_voxel_set_requests: dimension lookup failed; dropping",
            );
            return;
        };
        let Some((chunk, mut storage)) = chunk_index
            .get(chunk_pos)
            .and_then(|e| chunks.get_mut(e).ok())
        else {
            // Chunk unloaded or its palette query missed. This is
            // expected (a voxel update can arrive for a chunk that just
            // unloaded) and not an error.
            tracing::trace!(
                target: "voxel_update",
                ?chunk_pos,
                pos = ?request.pos,
                "apply_voxel_set_requests: chunk not present in SectionIndex; dropping",
            );
            return;
        };

        let old_state = storage
            .make_mut()
            .set(LocalPos::from(request.pos), request.new_state);
        if old_state == request.new_state {
            return;
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
    pub chunk_pos: SectionPos,
    pub block_pos: BlockPos,
    pub old_state: VoxelId,
    pub new_state: VoxelId,
    pub flags: BlockUpdateFlags,
}
