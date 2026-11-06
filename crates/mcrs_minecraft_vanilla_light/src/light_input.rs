use mcrs_minecraft_light_engine::world::Edit;
use mcrs_voxel::ColumnPos;

use crate::light_budget::LightBudget;

/// What a dimension tells its light stage.
pub(crate) enum LightInput {
    /// Edits, each scored against the players the stage knows last.
    Edits(Vec<Edit>),
    /// The columns the players stand in, to score the work against.
    Players(Vec<ColumnPos>),
    /// Columns to answer once the light around them has settled, with their light when the
    /// stage hands back nothing else.
    AwaitColumns(Vec<ColumnPos>),
    LightBudget(LightBudget),
}
