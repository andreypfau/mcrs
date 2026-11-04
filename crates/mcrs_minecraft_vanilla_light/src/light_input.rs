use mcrs_minecraft_light_engine::queue::Priority;
use mcrs_minecraft_light_engine::world::Edit;
use mcrs_voxel::ColumnPos;

use crate::light_budget::LightBudget;

/// An edit and how urgent its lighting is. Lower is more urgent.
pub(crate) struct QueuedEdit {
    pub(crate) edit: Edit,
    pub(crate) priority: Priority,
}

/// What a dimension tells its light stage.
pub(crate) enum LightInput {
    Edits(Vec<QueuedEdit>),
    /// The columns the players stand in, to re-score the waiting work against.
    Players(Vec<ColumnPos>),
    /// Columns to answer once the light around them has settled.
    AwaitColumns(Vec<ColumnPos>),
    /// Columns to answer with their light once the light around them has settled.
    AwaitLight(Vec<ColumnPos>),
    LightBudget(LightBudget),
}
