use crate::bus::{InboundPlayerDespawn, OutboundPlayerReleased};
use crate::game_profile::GameProfile;
use crate::persistence;
use crate::publish_player_tiles::publish_leave_message;
use crate::replicated::Replicated;
use crate::tile_events::TileEvents;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Query, Res, ResMut, With};
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

/// Per-dim consumer that despawns the in-dim player entity when an
/// `InboundPlayerDespawn` arrives for its session on a disconnect, so the
/// dimension stops streaming chunks toward that connection. The players near it hear that it
/// left.
pub fn despawn_inbound_player(
    mut reader: MessageReader<InboundPlayerDespawn>,
    index: Res<DimPlayerIndex>,
    players: Query<(Option<&GameProfile>, Option<&Replicated>), With<Player>>,
    mut events: Option<ResMut<TileEvents>>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        if let Some(&entity) = index.0.get(&msg.session)
            && let Ok((profile, replicated)) = players.get(entity)
        {
            if let (Some(profile), Some(replicated), Some(events)) =
                (profile, replicated, events.as_deref_mut())
            {
                publish_leave_message(events, replicated, profile);
            }
            commands.queue(move |world: &mut World| {
                persistence::write_player(world, entity);
                world.despawn(entity);
            });
        }
        // Queued after the save so the host never learns of the release before the file is written.
        let session = msg.session;
        commands.queue(move |world: &mut World| {
            world.write_message(OutboundPlayerReleased { session });
        });
    }
}
