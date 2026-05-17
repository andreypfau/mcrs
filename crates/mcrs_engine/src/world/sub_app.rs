use bevy_app::AppLabel;
use bevy_ecs::prelude::{Entity, Resource};

use crate::world::dimension::{DimensionId, DimensionTypeConfig};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct DimAppLabel(pub Entity);

#[derive(Debug, Clone)]
pub struct DimSpawnRequest {
    pub dimension_id: DimensionId,
    pub type_config: DimensionTypeConfig,
    pub has_sky: bool,
}

#[derive(Resource, Default)]
pub struct DimSpawnQueue(pub Vec<DimSpawnRequest>);

// Entries are allocated inside their owning sub-app world; the outer runner loop
// uses them as the key to `App::remove_sub_app(DimAppLabel(entity))`.
#[derive(Resource, Default)]
pub struct DimDespawnQueue(pub Vec<Entity>);
