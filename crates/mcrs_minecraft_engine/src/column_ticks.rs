//! The ticks that landed in the dimension this tick, from the data layer or from generation.

use bevy_ecs::resource::Resource;
use mcrs_minecraft_region_disk::column_ticks::ColumnTicks;
use mcrs_voxel::ColumnPos;

/// The ticks of a column whose sections landed in the dimension this tick, from the data layer
/// or from generation. The block simulation of the dimension takes them in the same tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandedColumnTicks {
    pub column: ColumnPos,
    pub ticks: ColumnTicks,
}

/// The ticks that landed this tick, in the order the columns landed.
#[derive(Resource, Debug, Default)]
pub struct LandedTicks(pub Vec<LandedColumnTicks>);

impl LandedTicks {
    pub fn land(&mut self, column: ColumnPos, ticks: ColumnTicks) {
        if !ticks.is_empty() {
            self.0.push(LandedColumnTicks { column, ticks });
        }
    }
}
