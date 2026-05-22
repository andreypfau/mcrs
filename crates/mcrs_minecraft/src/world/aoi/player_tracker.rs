//! Concrete `PlayerTracker` impl and the per-dim plugin that registers
//! its systems into `FixedPostUpdate`. The plugin also seeds
//! `AoiTickProbe` (used by the stationary-zero-work invariant test) and
//! installs an idempotent `FixedPreUpdate` system that ensures every
//! chunk-column entity carries a `PlayerObservers` Component before any
//! AoI write tries to mirror into it.

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_ecs::prelude::{IntoScheduleConfigs, Query};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{ScheduleConfigs, SystemSet};
use bevy_ecs::system::ScheduleSystem;
use mcrs_engine::aoi::{EntityTracker, TickInterval};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;

use crate::world::aoi::insert_player_observers::insert_player_observers_on_new_columns;
use crate::world::aoi::probe::AoiTickProbe;
use crate::world::aoi::update_own_pov::update_own_pov;
use crate::world::aoi::update_tracked_by::update_tracked_by;

/// Marker for the player AoI tracker. The trait surface is generic; this
/// unit struct is the single anchor point for `EntityTracker` impl + the
/// downstream `PlayerTrackerPlugin`.
pub struct PlayerTracker;

/// `SystemSet` covering both AoI systems (`update_own_pov` chained into
/// `update_tracked_by`). Living on its own set keeps the AoI work from
/// inadvertently parallelising against simulation systems that touch the
/// same Components.
#[derive(SystemSet, Clone, Default, Hash, PartialEq, Eq, Debug)]
pub struct PlayerTrackerSet;

/// Reserved per-tracker cache Resource. Empty by design: the per-player
/// `TrackedBy` Component and per-column `PlayerObservers` carry the
/// invariants. Kept around so future read-side caches (e.g., recipient
/// fan-out queues) have a typed home that does not require trait-surface
/// churn.
#[derive(Resource, Default)]
pub struct PlayerTrackerCache;

impl EntityTracker for PlayerTracker {
    type Entity = Player;
    type Cache = PlayerTrackerCache;
    type Set = PlayerTrackerSet;
    const CADENCE: TickInterval = TickInterval::Every;

    fn systems() -> ScheduleConfigs<ScheduleSystem> {
        (update_own_pov, update_tracked_by)
            .chain()
            .in_set(PlayerTrackerSet)
    }
}

/// Run-criterion that gates `PlayerTrackerSet` so neither AoI system
/// body executes on ticks where no player's `Transform` changed. The
/// stationary-zero-work invariant (AOI-04 + `aoi_stationary_zero_work.rs`)
/// asserts the AoI probe counters stay flat across stationary ticks; the
/// `Changed<Transform>` Query filter alone does NOT skip the system
/// body, only the iteration — so we hoist the same predicate up to the
/// schedule and skip the whole set when nothing moved.
pub fn on_changed_transform(query: Query<(), bevy_ecs::prelude::Changed<Transform>>) -> bool {
    !query.is_empty()
}

/// Per-dim plugin: registers `PlayerTrackerCache` + `AoiTickProbe`,
/// installs the `insert_player_observers_on_new_columns` seeder in
/// `FixedPreUpdate`, and registers the two AoI systems in
/// `FixedPostUpdate` gated by `on_changed_transform`.
pub struct PlayerTrackerPlugin;

impl Plugin for PlayerTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerTrackerCache>();
        app.init_resource::<AoiTickProbe>();
        app.add_systems(FixedPreUpdate, insert_player_observers_on_new_columns);
        app.add_systems(
            FixedPostUpdate,
            PlayerTracker::systems().run_if(on_changed_transform),
        );
        app.add_observer(crate::world::aoi::on_player_remove::on_player_remove);
    }
}
