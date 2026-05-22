//! Per-dim drain system for player removals arriving via the inbound
//! lifecycle bus. When a player disconnects or changes dimension, the
//! main-world disconnect path pushes an `InboundPlayerDespawn` message
//! into `PendingInboundLifecycle`; the extract closure shuttles it into
//! the per-dim sub-app's `Messages<InboundPlayerDespawn>` buffer; and
//! this system, running in `FixedPreUpdate`, consumes those messages and
//! evicts the corresponding in-dim `Player` entity from every column's
//! `PlayerObservers` set.
//!
//! A defensive helper `retain_live_observers` is exposed for use by
//! per-dim wire emitters that want a belt-and-braces liveness filter
//! before fan-out.

use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Entity, Query, With, Without};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::storage::column::Column;
use smallvec::SmallVec;

use crate::world::bus::InboundPlayerDespawn;
use crate::world::player_index::HostAnchorRef;

/// Per-dim drain: reads `InboundPlayerDespawn` messages, resolves each
/// `host_anchor` to the corresponding in-dim `Player` entity via
/// `HostAnchorRef`, and retains that entity out of every column's
/// `PlayerObservers`.
///
/// The query filter `(With<Column>, Without<Player>)` mirrors
/// `update_own_pov` and `update_tracked_by` — it makes the disjoint-borrow
/// claim explicit and prevents accidental matches on any future entity that
/// carries both `Column` and `PlayerObservers`.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::drain_inbound_player_despawn", skip_all)
)]
pub fn drain_inbound_player_despawn(
    mut despawn_msgs: MessageReader<InboundPlayerDespawn>,
    player_lookup: Query<(Entity, &HostAnchorRef), With<Player>>,
    mut columns: Query<&mut PlayerObservers, (With<Column>, Without<Player>)>,
) {
    for msg in despawn_msgs.read() {
        let host_anchor = msg.host_anchor;
        let in_dim_entity = player_lookup
            .iter()
            .find_map(|(e, anchor_ref)| (anchor_ref.0 == host_anchor).then_some(e));
        let Some(target) = in_dim_entity else {
            // No per-dim Player carries this host_anchor — happens when the
            // disconnect message arrives for a dim the player never reached
            // (mid-transit disconnect into the previous_dim bundle), or
            // before the per-dim spawn consumer has wired HostAnchorRef onto
            // the in-dim Player. Harmless: there is nothing to evict here.
            continue;
        };
        for mut obs in columns.iter_mut() {
            obs.0.retain(|e| *e != target);
        }
    }
}

/// Defensive helper: filter an `observer_entities` snapshot against a
/// live-player query before fan-out. Used by per-dim wire emitters
/// (`update_client_blocks_per_dim`) as belt-and-braces protection
/// against any future code path that mutates `PlayerObservers` without
/// going through the drain above. Cost: one Query iteration per
/// emit-site invocation; acceptable because the live-player set is small
/// and the emit site already pays a Query cost for `&PlayerObservers`.
pub fn retain_live_observers(
    observer_entities: &mut SmallVec<[Entity; 8]>,
    live_players: &Query<Entity, With<Player>>,
) {
    observer_entities.retain(|entity| live_players.get(*entity).is_ok());
}
