use glam::DVec3;
use glam::IVec3;
use mcrs_voxel::BlockStateId;

use crate::fall::AtLanding;
use crate::fall::Fall;
use crate::fall::ItemDrop;
use crate::falling_block_table::FallingBlockTable;
use crate::state_flags::StateFlags;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    pub leftover: BlockStateId,
    pub position: DVec3,
    pub falling_block: Fall,
}

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
