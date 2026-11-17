use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::TickPriority;
use mcrs_voxel::{BlockPos, SectionPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredTick<T> {
    pub pos: BlockPos,
    pub kind: T,
    pub delay: i32,
    pub priority: TickPriority,
}

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

    pub fn retain_rows(&mut self, rows: &[i32]) {
        let kept = |pos: BlockPos| rows.contains(&SectionPos::from(pos).y);
        self.block.retain(|tick| kept(tick.pos));
        self.fluid.retain(|tick| kept(tick.pos));
    }
}
