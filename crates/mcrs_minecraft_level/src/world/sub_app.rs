use bevy_app::AppLabel;
use bevy_ecs::prelude::{Entity, Resource};

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_keys as keys;

use crate::world::dimension::DimensionTypeConfig;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct DimAppLabel(pub Entity);

#[derive(Debug, Clone)]
pub struct DimSpawnRequest {
    pub dimension: ResourceKey<keys::Dimension>,
    pub type_config: DimensionTypeConfig,
    pub has_sky: bool,
}

#[derive(Resource, Default)]
pub struct DimSpawnQueue(pub Vec<DimSpawnRequest>);

/// Queue of host-world `DimSubAppHandle` label entities awaiting sub-app teardown.
/// Entries are the `Entity` values used as `DimAppLabel(Entity)` keys when the
/// sub-app was inserted — **not** the `Dimension` entity that lives inside the
/// sub-app's `World`. The outer runner loop drains this queue and calls
/// `App::remove_sub_app(DimAppLabel(entity))` on each, then despawns the
/// host-side handle entity.
#[derive(Resource, Default)]
pub struct DimDespawnQueue(pub Vec<Entity>);
