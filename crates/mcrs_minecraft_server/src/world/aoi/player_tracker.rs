//! The per-dim plugin that registers the player AoI systems into
//! `FixedPostUpdate`. The plugin also seeds
//! `AoiTickProbe` (used by the stationary-zero-work invariant test) and
//! keeps each column's `PlayerObservers` in line with the columns players
//! hold.

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_ecs::prelude::{IntoScheduleConfigs, Query};
use bevy_ecs::schedule::SystemSet;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::voxel_update::VoxelUpdateSet;

use crate::world::aoi::mirror::{
    ColumnHeld, announce_held_columns, mirror_held_columns, withdraw_held_columns,
};
use crate::world::aoi::probe::AoiTickProbe;
use crate::world::aoi::update_tracked_by::update_tracked_by;

/// `SystemSet` covering the AoI system. Living on its own set keeps the AoI
/// work from inadvertently parallelising against simulation systems that
/// touch the same Components.
#[derive(SystemSet, Clone, Default, Hash, PartialEq, Eq, Debug)]
pub struct PlayerTrackerSet;

/// Run-criterion that gates `PlayerTrackerSet` so the AoI system body does
/// not execute on ticks where no player's `Transform` changed. The
/// stationary-zero-work invariant (`aoi_stationary_zero_work.rs`) asserts
/// the AoI probe counter stays flat across stationary ticks; the
/// `Changed<Transform>` Query filter alone does NOT skip the system
/// body, only the iteration — so we hoist the same predicate up to the
/// schedule and skip the whole set when nothing moved.
pub fn on_changed_transform(query: Query<(), bevy_ecs::prelude::Changed<Transform>>) -> bool {
    !query.is_empty()
}

/// Per-dim plugin: registers `AoiTickProbe`, the
/// despawn drain in `FixedPreUpdate`, the observer mirror, and the AoI
/// system in `FixedPostUpdate` gated by `on_changed_transform`.
pub struct PlayerTrackerPlugin;

impl Plugin for PlayerTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AoiTickProbe>();
        app.add_message::<ColumnHeld>();
        app.add_observer(announce_held_columns);
        app.add_observer(withdraw_held_columns);
        // Must stay in FixedPreUpdate: the drain owns PlayerLeftView emission
        // for removed players. update_tracked_by (FixedPostUpdate) never sees
        // the left-view transition because the drain clears TrackedBy before
        // update_tracked_by runs.
        app.add_systems(
            FixedPreUpdate,
            crate::world::aoi::drain_player_despawn::drain_inbound_player_despawn,
        );
        app.add_systems(
            FixedPostUpdate,
            mirror_held_columns
                .before(PlayerTrackerSet)
                .before(VoxelUpdateSet::NetworkSync),
        );
        app.add_systems(
            FixedPostUpdate,
            update_tracked_by
                .in_set(PlayerTrackerSet)
                .run_if(on_changed_transform),
        );
    }
}
