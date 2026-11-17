use bevy_ecs::resource::Resource;
use mcrs_minecraft_region_disk::column_ticks::ColumnTicks;
use mcrs_voxel::ColumnPos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandedColumnTicks {
    pub column: ColumnPos,
    pub ticks: ColumnTicks,
}

#[derive(Resource, Debug, Default)]
pub struct LandedTicks(pub Vec<LandedColumnTicks>);

impl LandedTicks {
    pub fn land(&mut self, column: ColumnPos, ticks: ColumnTicks) {
        if !ticks.is_empty() {
            self.0.push(LandedColumnTicks { column, ticks });
        }
    }
}
