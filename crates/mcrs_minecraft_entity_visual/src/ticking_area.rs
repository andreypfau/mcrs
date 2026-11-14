use bevy_ecs::resource::Resource;
use mcrs_voxel::ColumnPos;

/// The columns whose entities tick.
pub trait TickingArea: Resource {
    /// Whether the entities of `column` tick.
    fn holds(&self, column: ColumnPos) -> bool;
}
