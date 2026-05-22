//! Idempotent seeder: attaches a default `PlayerObservers` Component to
//! any chunk-column entity that does not yet carry one. Runs in each
//! DimSubApp's `FixedPreUpdate` so `update_own_pov` (in
//! `FixedPostUpdate`) is guaranteed to find the Component on every
//! column it tries to mirror-write into.

use bevy_ecs::prelude::{Commands, Entity, Query, With, Without};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::world::storage::column::Column;

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::insert_player_observers", skip_all)
)]
pub fn insert_player_observers_on_new_columns(
    query: Query<Entity, (With<Column>, Without<PlayerObservers>)>,
    mut commands: Commands,
) {
    for entity in query.iter() {
        commands.entity(entity).insert(PlayerObservers::default());
    }
}
