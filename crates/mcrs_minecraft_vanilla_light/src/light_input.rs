use mcrs_minecraft_light_engine::world::Edit;
use mcrs_voxel::ColumnPos;

use crate::light_budget::LightBudget;

pub(crate) enum LightInput {
    Edits(Vec<Edit>),
    Players(Vec<ColumnPos>),
    AwaitColumns(Vec<ColumnPos>),
    LightBudget(LightBudget),
}
