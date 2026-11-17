use glam::IVec3;
use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fall {
    pub carried: BlockStateId,
    pub item_drop: ItemDrop,
    pub at_landing: AtLanding,
    pub start: IVec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemDrop {
    Allowed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtLanding {
    Settle,
    Break,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickStart {
    Remove,
    Move,
}

impl Fall {
    #[must_use]
    pub fn begin_tick(&self) -> TickStart {
        if self.carried == BlockStateId::AIR {
            TickStart::Remove
        } else {
            TickStart::Move
        }
    }
}
