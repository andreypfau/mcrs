use crate::world::storage::chunk::ChunkIndex;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter, Messages};
use bevy_ecs::prelude::{Commands, Component, Query};
use bevy_ecs::query::{With, Without};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_storage::{VoxelId, VoxelPalette};
use rustc_hash::FxHashSet;
use std::marker::PhantomData;

/// The only question the engine asks of a game's update flags.
pub trait VoxelUpdateFlags: Copy + Send + Sync + 'static {
    fn notifies_clients(&self) -> bool;
}

impl VoxelUpdateFlags for bool {
    fn notifies_clients(&self) -> bool {
        *self
    }
}

pub type SectionVoxels = VoxelPalette<VoxelId, { BLOCKS::SIZE }>;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VoxelUpdateSet {
    ApplyChanges,
    NetworkSync,
}

pub struct VoxelUpdatePlugin<F: VoxelUpdateFlags>(PhantomData<fn() -> F>);

impl<F: VoxelUpdateFlags> Default for VoxelUpdatePlugin<F> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<F: VoxelUpdateFlags> Plugin for VoxelUpdatePlugin<F> {
    fn build(&self, app: &mut bevy_app::App) {
        // Messages<VoxelSetRequest> and Messages<VoxelPlaced> are
        // registered by the caller (the per-dim sub-app builder) before
        // this plugin runs. Registering them here would re-export the
        // buffers if the plugin were ever added host-side by mistake,
        // hiding the per-dim invariant.
        debug_assert!(
            app.world()
                .contains_resource::<Messages<VoxelSetRequest<F>>>(),
            "VoxelUpdatePlugin requires Messages<VoxelSetRequest> registered by the per-dim sub-app builder before add_plugins",
        );
        debug_assert!(
            app.world().contains_resource::<Messages<VoxelPlaced<F>>>(),
            "VoxelUpdatePlugin requires Messages<VoxelPlaced> registered by the per-dim sub-app builder before add_plugins",
        );
        app.configure_sets(FixedUpdate, VoxelUpdateSet::ApplyChanges);
        app.configure_sets(FixedPostUpdate, VoxelUpdateSet::NetworkSync);
        app.add_systems(FixedUpdate, add_changes_set);
        app.add_systems(
            FixedUpdate,
            apply_voxel_set_requests::<F>.in_set(VoxelUpdateSet::ApplyChanges),
        );
    }
}

#[derive(Message)]
pub struct VoxelSetRequest<F: VoxelUpdateFlags> {
    pub dimension: Entity,
    pub pos: BlockPos,
    pub new_state: VoxelId,
    pub flags: F,
    pub recursion_left: i16,
}

#[derive(Default, Component)]
pub struct ChunkVoxelChanges {
    pub changes: FxHashSet<BlockPos>,
}

fn add_changes_set(
    query: Query<Entity, (With<SectionVoxels>, Without<ChunkVoxelChanges>)>,
    mut commands: Commands,
) {
    for entity in query.iter() {
        commands.entity(entity).insert(ChunkVoxelChanges::default());
    }
}

pub fn apply_voxel_set_requests<F: VoxelUpdateFlags>(
    mut reader: MessageReader<VoxelSetRequest<F>>,
    dimensions: Query<&ChunkIndex>,
    mut chunks: Query<(Entity, &mut SectionVoxels, &mut ChunkVoxelChanges)>,
    mut writer: MessageWriter<VoxelPlaced<F>>,
) {
    reader.read().for_each(|request| {
        let chunk_pos = ChunkPos::from(request.pos);

        let Ok(chunk_index) = dimensions.get(request.dimension) else {
            // Stale dimension Entity. Treated as suspicious because the
            // dimension typically outlives a single FixedUpdate; the
            // most common cause is a bus race between dim despawn and
            // VoxelSetRequest delivery.
            tracing::warn!(
                target: "voxel_update",
                dimension = ?request.dimension,
                pos = ?request.pos,
                "apply_voxel_set_requests: dimension lookup failed; dropping",
            );
            return;
        };
        let Some((chunk, mut storage, mut changes)) = chunk_index
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
                "apply_voxel_set_requests: chunk not present in ChunkIndex; dropping",
            );
            return;
        };

        let old_state = storage.set(request.pos, request.new_state);
        if old_state == request.new_state {
            return;
        }
        if request.flags.notifies_clients() {
            changes.changes.insert(request.pos);
        }

        writer.write(VoxelPlaced {
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
pub struct VoxelPlaced<F: VoxelUpdateFlags> {
    pub chunk: Entity,
    pub chunk_pos: ChunkPos,
    pub block_pos: BlockPos,
    pub old_state: VoxelId,
    pub new_state: VoxelId,
    pub flags: F,
}
