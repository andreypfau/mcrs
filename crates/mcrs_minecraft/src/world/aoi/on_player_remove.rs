//! Cleanup hook for Player Component removal. Bevy recycles Entity
//! slots (generation increments), so a stale Entity reference in a
//! column's PlayerObservers can deliver per-block packets to a
//! different live entity that later occupies the same slot. The
//! On<Remove, Player> observer iterates every column's
//! PlayerObservers in the per-dim sub-app and retains out the
//! despawned Entity in the same despawn flush. A defensive helper
//! `retain_live_observers` is exposed for use by per-dim wire
//! emitters that want a belt-and-braces liveness filter before
//! fan-out.

use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Entity, Query, With};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::storage::column::Column;
use smallvec::SmallVec;

/// On<Remove, Player> observer. Fires synchronously when the Player
/// Component is removed (via explicit remove or entity despawn).
/// Iterates every Column entity in this per-dim sub-app's World and
/// retains the despawned Entity out of each PlayerObservers set.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::on_player_remove", skip_all)
)]
pub fn on_player_remove(
    trigger: On<Remove, Player>,
    mut columns: Query<&mut PlayerObservers, With<Column>>,
) {
    let removed = trigger.event().entity;
    for mut obs in columns.iter_mut() {
        obs.0.retain(|e| *e != removed);
    }
}

/// Defensive helper: filter an `observer_entities` snapshot against a
/// live-player query before fan-out. Used by per-dim wire emitters
/// (`update_client_blocks_per_dim`) as belt-and-braces protection
/// against any future code path that mutates PlayerObservers without
/// going through the observer above (e.g., a manual edit during a
/// migration). Cost: one Query iteration per emit-site invocation;
/// acceptable because the live-player set is small and the emit
/// site already pays a Query cost for `&PlayerObservers`.
pub fn retain_live_observers(
    observer_entities: &mut SmallVec<[Entity; 8]>,
    live_players: &Query<Entity, With<Player>>,
) {
    observer_entities.retain(|entity| live_players.get(*entity).is_ok());
}
