use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Rollback {
    pub(crate) edits: Vec<Edit>,
    pub(crate) ack: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Edit {
    pub(crate) block: BlockPos,
    pub(crate) position: DVec3,
    pub(crate) state: Option<BlockStateId>,
}

impl Rollback {
    pub(crate) fn acked(&self, applied: u64) -> bool {
        self.ack.is_some_and(|ack| ack < applied)
    }

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

    pub(crate) fn unsettled(&self, applied: u64, bounds: &Aabb) -> bool {
        !self.acked(applied)
            && self.edits.iter().any(|edit| {
                let low = edit.block.as_dvec3();
                edit.state.is_none() && bounds.intersects(&Aabb::new(low, low + DVec3::ONE))
            })
    }
}
