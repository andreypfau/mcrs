use bevy::prelude::*;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol::packets::common::serverbound::Pong;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPing;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPong;

pub(crate) fn add_observers(app: &mut App) {
    app.add_observer(on_ping);
}

/// Answers a `ping` with a `pong` of the same id. A server that waits too long for it ends the
/// session.
fn on_ping(event: On<Received<ClientboundPing>>, mut connections: Query<&mut ClientConnection>) {
    let (Some(ping), Ok(mut connection)) = (event.packet(), connections.get_mut(event.entity))
    else {
        return;
    };
    connection.write_packet(&ServerboundPong(Pong { payload: ping.id }));
}
