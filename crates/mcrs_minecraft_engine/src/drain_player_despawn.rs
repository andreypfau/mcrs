//! Per-dim drain system for player removals arriving via the inbound
//! lifecycle bus. When a player disconnects or changes dimension, the
//! main-world disconnect path pushes an `InboundPlayerDespawn` message
//! into `PendingInboundLifecycle`; the extract closure shuttles it into
//! the per-dim sub-app's `Messages<InboundPlayerDespawn>` buffer; and
//! this system, running in `FixedPreUpdate`, takes the view of the
//! corresponding in-dim `Player` entity. Its viewers lose it when the
//! entity despawns, through the entity tracker.
//!
//! A defensive helper `retain_live_observers` is exposed for use by
//! per-dim wire emitters that want a belt-and-braces liveness filter
//! before fan-out.

use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Entity, Query, Res, With};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;
use smallvec::SmallVec;

use crate::bus::InboundPlayerDespawn;
use crate::column_view::ColumnView;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

/// Per-dim drain: reads `InboundPlayerDespawn` messages and takes the `ColumnView` of the
/// resolved in-dim `Player`, which hands back its tickets and withdraws it from every column it
/// held.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::drain_inbound_player_despawn", skip_all)
)]
pub fn drain_inbound_player_despawn(
    mut despawn_msgs: MessageReader<InboundPlayerDespawn>,
    index: Res<DimPlayerIndex>,
    players: Query<(), With<Player>>,
    mut commands: Commands,
) {
    for msg in despawn_msgs.read() {
        // No player of this dimension has the session yet when the disconnect came before the
        // spawn of the player here. There is nothing to evict.
        let Some(&target) = index.0.get(&msg.session) else {
            continue;
        };
        if !players.contains(target) {
            continue;
        }
        commands
            .entity(target)
            .try_remove::<(ColumnView, SentChunkView)>();
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
