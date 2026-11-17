use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Query, Res, With};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;

use crate::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

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
        let Some(&target) = index.0.get(&msg.session) else {
            continue;
        };
        if !players.contains(target) {
            continue;
        }
        commands.entity(target).try_remove::<SentChunkView>();
    }
}
