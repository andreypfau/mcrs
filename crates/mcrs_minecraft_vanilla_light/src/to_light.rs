use mcrs_minecraft_light_engine::queue::Priority;
use mcrs_minecraft_light_engine::world::Edit;
use mcrs_voxel::ColumnPos;

use crate::intake_budget::IntakeBudget;
use crate::light_budget::LightBudget;

/// An edit and how urgent its lighting is. Lower is more urgent.
pub(crate) struct QueuedEdit {
    pub(crate) edit: Edit,
    pub(crate) priority: Priority,
}

/// What a dimension tells its light actor.
pub(crate) enum ToLight {
    Edits(Vec<QueuedEdit>),
    /// The columns the players stand in, to re-score the waiting work against.
    Players(Vec<ColumnPos>),
    /// Columns to answer once the light around them has settled.
    AwaitColumns(Vec<ColumnPos>),
    LightBudget(LightBudget),
    IntakeBudget(IntakeBudget),
}
