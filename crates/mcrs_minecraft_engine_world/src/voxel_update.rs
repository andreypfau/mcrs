use crate::owned_columns::OwnedColumns;
use crate::world::storage::view::WorldViewMut;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter, Messages};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Res, ResMut};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_voxel::SectionPos;
use mcrs_voxel::{BlockPos, BlockStateId};

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

/// A block write that one cell of a world hands another: a write into a column of the other
/// cell, or a change of a column of its own for the halo of the other cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellBlockWrite {
    pub pos: BlockPos,
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
    pub recursion_left: i16,
}

/// The writes of this tick into columns that another cell owns, for their owners. Only a world
/// with [`OwnedColumns`] holds it.
#[derive(Resource, Debug, Default)]
pub struct ForeignBlockWrites(pub Vec<CellBlockWrite>);

/// The changes of the halo that the owners sent for this tick. They apply before the requests
/// of the tick, and only change the copy: no neighbour hears of them.
#[derive(Resource, Debug, Default)]
pub struct HaloBlockChanges(pub Vec<HaloBlockChange>);

#[derive(Clone, Copy, Debug)]
pub struct HaloBlockChange {
    pub dimension: Entity,
    pub write: CellBlockWrite,
}

pub fn apply_voxel_set_requests(
    mut reader: MessageReader<BlockSetRequest>,
    mut blocks: WorldViewMut,
    mut writer: MessageWriter<BlockPlaced>,
    owned: Option<Res<OwnedColumns>>,
    mut foreign: Option<ResMut<ForeignBlockWrites>>,
    mut halo: Option<ResMut<HaloBlockChanges>>,
) {
    if let Some(halo) = halo.as_mut() {
        apply_halo_changes(&mut blocks, &mut writer, &mut halo.0);
    }
    for request in reader.read() {
        let write = CellBlockWrite {
            pos: request.pos,
            new_state: request.new_state,
            flags: request.flags,
            recursion_left: request.recursion_left,
        };
        if let Some(owned) = owned.as_deref()
            && !owned.owns_block(request.pos)
        {
            if let Some(foreign) = foreign.as_mut() {
                foreign.0.push(write);
            }
            continue;
        }
        apply_write(&mut blocks, &mut writer, request.dimension, write);
    }
}

/// Applies the changes of the halo that the owners sent, before any write of the tick. They only
/// change the copy: no neighbour hears of them.
pub fn apply_halo_changes(
    blocks: &mut WorldViewMut,
    writer: &mut MessageWriter<BlockPlaced>,
    changes: &mut Vec<HaloBlockChange>,
) {
    for HaloBlockChange { dimension, write } in changes.drain(..) {
        apply_write(blocks, writer, dimension, write);
    }
}

/// Stores `write` in its section and reports the change, with no neighbour or shape update.
pub fn apply_write(
    blocks: &mut WorldViewMut,
    writer: &mut MessageWriter<BlockPlaced>,
    dimension: Entity,
    write: CellBlockWrite,
) {
    let Some(mut view) = blocks.get_mut(dimension) else {
        // Stale dimension Entity. Treated as suspicious because the
        // dimension typically outlives a single FixedUpdate; the
        // most common cause is a bus race between dim despawn and
        // BlockSetRequest delivery.
        tracing::warn!(
            target: "voxel_update",
            ?dimension,
            pos = ?write.pos,
            "apply_voxel_set_requests: dimension lookup failed; dropping",
        );
        return;
    };
    let old_state = match view.set_block(write.pos, write.new_state) {
        Ok(old_state) => old_state,
        Err(why) => {
            // A voxel update can arrive for a section that just unloaded;
            // this is expected and not an error.
            tracing::trace!(
                target: "voxel_update",
                %why,
                pos = ?write.pos,
                "apply_voxel_set_requests: dropping",
            );
            return;
        }
    };
    if old_state == write.new_state {
        return;
    }

    let chunk_pos = SectionPos::from(write.pos);
    let Some(chunk) = view.section_entity(chunk_pos) else {
        return;
    };
    writer.write(BlockPlaced {
        chunk,
        chunk_pos,
        block_pos: write.pos,
        old_state,
        new_state: write.new_state,
        flags: write.flags,
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
