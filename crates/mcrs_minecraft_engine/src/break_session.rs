use bevy_ecs::component::Component;
use mcrs_minecraft_block_break::Dig;
use mcrs_minecraft_core::Direction;
use mcrs_voxel::{BlockPos, BlockStateId};

/// The survival dig of a player, and the break that a finish action before a progress of 0.7
/// delays. Every player has one; an idle player holds neither.
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

/// The progress at one position, and the stage that the observers have for it.
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

/// It ends when the block is no longer `state`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DelayedBreak {
    pub(crate) crack: Crack,
    pub(crate) state: BlockStateId,
}
