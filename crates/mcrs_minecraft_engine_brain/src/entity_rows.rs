use mcrs_minecraft_entity_rows::RowStore;

use crate::MobColumn;

/// The rows of the server entities of a dimension world, with the column of the mobs with AI.
pub type EntityRows = RowStore<MobColumn>;
