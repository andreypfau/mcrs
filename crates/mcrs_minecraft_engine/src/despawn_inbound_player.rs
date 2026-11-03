use crate::bus::{InboundPlayerDespawn, OutboundPlayerReleased};
use crate::persistence;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Query, Res, With};
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

/// Per-dim consumer that despawns the in-dim player entity when an
/// `InboundPlayerDespawn` arrives for its session on a disconnect, so the
/// dimension stops streaming chunks toward that connection.
pub fn despawn_inbound_player(
    mut reader: MessageReader<InboundPlayerDespawn>,
    index: Res<DimPlayerIndex>,
    players: Query<(), With<Player>>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        if let Some(&entity) = index.0.get(&msg.session)
            && players.contains(entity)
        {
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
