//! Per-dim drain system for player removals arriving via the inbound
//! lifecycle bus. When a player disconnects or changes dimension, the
//! main-world disconnect path pushes an `InboundPlayerDespawn` message
//! into `PendingInboundLifecycle`; the extract closure shuttles it into
//! the per-dim sub-app's `Messages<InboundPlayerDespawn>` buffer; and
//! this system, running in `FixedPreUpdate`, takes the view of the
//! corresponding in-dim `Player` entity. Its viewers lose it when the
//! entity despawns, through its despawn in the tile deltas.

use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Query, Res, With};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;

use crate::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

/// Per-dim drain: reads `InboundPlayerDespawn` messages and takes the view of the resolved
/// in-dim `Player`, so the muxer stops serving it.
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
        commands.entity(target).try_remove::<SentChunkView>();
    }
}
