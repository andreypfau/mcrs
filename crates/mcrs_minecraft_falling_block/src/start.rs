use glam::DVec3;
use glam::IVec3;
use mcrs_voxel::BlockStateId;

use crate::fall::AtLanding;
use crate::fall::Fall;
use crate::fall::ItemDrop;
use crate::falling_block_table::FallingBlockTable;
use crate::state_flags::StateFlags;

/// The start of a fall (FBK-SPN-001): the state that the host sets at the block position, and the
/// falling block that it adds with the velocity 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    /// The block of the fluid of the state, set with the flags 3.
    pub leftover: BlockStateId,
    /// The position of the new falling block: the bottom centre of the block position.
    pub position: DVec3,
    /// The values of the new falling block.
    pub falling_block: Fall,
}

/// The start of a fall of `state` at the block position `block` (FBK-SPN-001).
#[must_use]
pub fn start(table: &FallingBlockTable, block: IVec3, state: BlockStateId) -> Start {
    let at_landing = if table.flags(state).contains(StateFlags::BREAKS_AFTER_FALL) {
        AtLanding::Break
    } else {
        AtLanding::Settle
    };
    Start {
        leftover: table.leftover(state),
        position: block.as_dvec3() + DVec3::new(0.5, 0.0, 0.5),
        falling_block: Fall {
            carried: table.dry(state),
            item_drop: ItemDrop::Allowed,
            at_landing,
            start: block,
        },
    }
}
