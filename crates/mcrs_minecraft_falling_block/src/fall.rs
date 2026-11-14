use glam::IVec3;
use mcrs_voxel::BlockStateId;

/// The values that a falling block adds to an entity (FBK-DAT-001). The position, the velocity and
/// the age are the host's, beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fall {
    /// The block that falls.
    pub carried: BlockStateId,
    /// The drop flag (FBK-TCK-004, FBK-LND-006).
    pub item_drop: ItemDrop,
    /// The break flag (FBK-LND-005).
    pub at_landing: AtLanding,
    /// The block position at which the fall started, the entity data entry of FBK-DAT-003.
    pub start: IVec3,
}

/// The drop flag: whether a falling block that does not land becomes an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemDrop {
    Allowed,
    Cancelled,
}

/// The break flag: whether a falling block makes a block at its landing or breaks (FBK-LND-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtLanding {
    Settle,
    Break,
}

/// The first decision of a tick, before the motion (FBK-TCK-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickStart {
    /// The carried state is `air`: the host removes the falling block, nothing drops.
    Remove,
    /// The host adds 1 to the age and applies the motion of the category "falling block".
    Move,
}

impl Fall {
    /// Step 1 of FBK-TCK-001: a falling block of `air` ends.
    #[must_use]
    pub fn begin_tick(&self) -> TickStart {
        if self.carried == BlockStateId::AIR {
            TickStart::Remove
        } else {
            TickStart::Move
        }
    }
}
