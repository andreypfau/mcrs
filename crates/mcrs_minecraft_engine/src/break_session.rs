use bevy_ecs::component::Component;
use mcrs_minecraft_block_break::Dig;
use mcrs_minecraft_core::Direction;
use mcrs_voxel::{BlockPos, BlockStateId};

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct BreakSession {
    pub(crate) dig: Option<ActiveDig>,
    pub(crate) delayed: Option<DelayedBreak>,
}

impl BreakSession {
    pub(crate) fn is_idle(&self) -> bool {
        self.dig.is_none() && self.delayed.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Crack {
    pub(crate) pos: BlockPos,
    pub(crate) dig: Dig,
    pub(crate) sent_stage: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ActiveDig {
    pub(crate) crack: Crack,
    pub(crate) face: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DelayedBreak {
    pub(crate) crack: Crack,
    pub(crate) state: BlockStateId,
}
