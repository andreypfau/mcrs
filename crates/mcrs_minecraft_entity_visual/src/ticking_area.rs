use bevy_ecs::resource::Resource;
use mcrs_voxel::ColumnPos;

pub trait TickingArea: Resource {
    fn holds(&self, column: ColumnPos) -> bool;
}
