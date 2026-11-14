use glam::IVec3;
use mcrs_voxel::BlockStateId;

/// The values that a falling block adds to an entity (FBK-DAT-001). The position and the
/// velocity are the host's, beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FallingBlock {
    pub carried: BlockStateId,
    pub age: i32,
    pub item_drop: ItemDrop,
    pub at_landing: AtLanding,
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
    /// The host applies the motion of the category "falling block".
    Move,
}

impl FallingBlock {
    /// Steps 1 and 2 of FBK-TCK-001: a falling block of `air` ends, any other one ages by 1. The
    /// age stops at its maximum.
    pub fn begin_tick(&mut self) -> TickStart {
        if self.carried == BlockStateId::AIR {
            return TickStart::Remove;
        }
        self.age = self.age.saturating_add(1);
        TickStart::Move
    }
}
