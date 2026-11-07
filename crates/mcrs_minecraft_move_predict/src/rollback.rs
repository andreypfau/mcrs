use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

/// The block edits of the client that wait for their `block_changed_ack`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Rollback {
    pub(crate) edits: Vec<Edit>,
    /// The number of the `block_changed_ack` change, once written.
    pub(crate) ack: Option<u64>,
}

/// An edited block and the position of the client at the first request that changed it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Edit {
    pub(crate) block: BlockPos,
    pub(crate) position: DVec3,
    /// The state of the last edit; `None` when the server cannot compute it.
    pub(crate) state: Option<BlockStateId>,
}

impl Rollback {
    /// True when a prefix of `applied` changes holds the `block_changed_ack`.
    pub(crate) fn acked(&self, applied: u64) -> bool {
        self.ack.is_some_and(|ack| ack < applied)
    }

    /// Before the ack, the client has the state of its edit at each edited block; from the ack
    /// on, the changes give the kept state.
    pub(crate) fn cover(&self, applied: u64, blocks: &mut FxHashMap<BlockPos, BlockStateId>) {
        if self.acked(applied) {
            return;
        }
        for edit in &self.edits {
            if let Some(state) = edit.state {
                blocks.insert(edit.block, state);
            }
        }
    }

    /// True when `bounds` touches an edited block whose state the server cannot compute, in a
    /// prefix of `applied` changes without the ack.
    pub(crate) fn unsettled(&self, applied: u64, bounds: &Aabb) -> bool {
        !self.acked(applied)
            && self.edits.iter().any(|edit| {
                let low = edit.block.as_dvec3();
                edit.state.is_none() && bounds.intersects(&Aabb::new(low, low + DVec3::ONE))
            })
    }
}
