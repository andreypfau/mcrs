use crate::bus::{InboundPlayerDespawn, OutboundPlayerReleased};
use crate::persistence;
use crate::session::HostAnchor;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Query, ResMut, With};
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;

/// Per-dim consumer that despawns the in-dim player entity when an
/// `InboundPlayerDespawn` arrives for its host anchor on a disconnect, so the
/// dimension stops streaming chunks toward that connection.
pub fn despawn_inbound_player(
    mut reader: MessageReader<InboundPlayerDespawn>,
    players: Query<(Entity, &HostAnchor), With<Player>>,
    mut commands: Commands,
    mut dim_index: ResMut<DimPlayerIndex>,
) {
    for msg in reader.read() {
        dim_index.0.remove(&msg.session);
        for (entity, anchor) in players.iter() {
            if anchor.0 == msg.host_anchor {
                commands.queue(move |world: &mut World| {
                    persistence::write_player(world, entity);
                    world.despawn(entity);
                });
            }
        }
        // Queued after the save so the host never learns of the release before the file is written.
        let session = msg.session;
        commands.queue(move |world: &mut World| {
            world.write_message(OutboundPlayerReleased { session });
        });
    }
}
