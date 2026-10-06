use bevy_app::AppLabel;
use bevy_ecs::prelude::{Entity, Resource};

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_registry::Id;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct DimAppLabel(pub Entity);

#[derive(Debug, Clone)]
pub struct DimSpawnRequest {
    pub dimension: ResourceKey<mcrs_minecraft_dimension::Dimension>,
    pub dimension_type: Id<DimensionType>,
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
