use glam::DVec3;
use mcrs_voxel::BlockPos;

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
}
