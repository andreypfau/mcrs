//! The scheduled ticks of a column while no queue holds them: in the data layer, or between
//! generation and the simulation of the dimension.

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::TickPriority;
use mcrs_voxel::{BlockPos, SectionPos};

/// One scheduled tick of a column at rest. Its delay counts from the game time at which a queue
/// takes it back, so the time that the column is cold does not count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredTick<T> {
    pub pos: BlockPos,
    pub kind: T,
    /// The due tick minus the game time of the save. It is negative for an overdue tick.
    pub delay: i32,
    pub priority: TickPriority,
}

/// The scheduled block ticks and fluid ticks of one column, each list in the order in which the
/// ticks run when their due tick and priority are equal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnTicks {
    pub block: Vec<StoredTick<Id<Block>>>,
    pub fluid: Vec<StoredTick<Id<Fluid>>>,
}

impl ColumnTicks {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.block.is_empty() && self.fluid.is_empty()
    }

    /// Only the ticks in the section rows of `rows`.
    pub fn retain_rows(&mut self, rows: &[i32]) {
        let kept = |pos: BlockPos| rows.contains(&SectionPos::from(pos).y);
        self.block.retain(|tick| kept(tick.pos));
        self.fluid.retain(|tick| kept(tick.pos));
    }
}
