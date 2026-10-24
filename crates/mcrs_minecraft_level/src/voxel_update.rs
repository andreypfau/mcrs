use crate::world::storage::view::WorldViewMut;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter, Messages};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_core::{BlockPos, BlockStateId};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_volume::BlockVolumeMut;

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
        app.configure_sets(
            FixedUpdate,
            VoxelUpdateSet::ApplyChanges.in_set(crate::tick::TickSet::WorldWrite),
        );
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
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
    pub recursion_left: i16,
}

pub fn apply_voxel_set_requests(
    mut reader: MessageReader<BlockSetRequest>,
    mut blocks: WorldViewMut,
    mut writer: MessageWriter<BlockPlaced>,
) {
    reader.read().for_each(|request| {
        let Some(mut dimension) = blocks.get_mut(request.dimension) else {
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
        let old_state = match dimension.set_block(request.pos, request.new_state) {
            Ok(old_state) => old_state,
            Err(why) => {
                // A voxel update can arrive for a section that just unloaded;
                // this is expected and not an error.
                tracing::trace!(
                    target: "voxel_update",
                    %why,
                    pos = ?request.pos,
                    "apply_voxel_set_requests: dropping",
                );
                return;
            }
        };
        if old_state == request.new_state {
            return;
        }

        let chunk_pos = SectionPos::from(request.pos);
        let Some(chunk) = dimension.section_entity(chunk_pos) else {
            return;
        };
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
    pub old_state: BlockStateId,
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
}
