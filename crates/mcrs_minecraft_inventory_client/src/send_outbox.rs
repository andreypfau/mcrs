use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_protocol::WritePacket;

use crate::outbox::Outbox;
use crate::outgoing::Outgoing;

pub(crate) fn send_outbox(
    mut outbox: ResMut<Outbox>,
    connection: Option<Single<(&mut ClientConnection, &ConnectionState)>>,
) {
    let Some((mut connection, _)) = connection
        .map(Single::into_inner)
        .filter(|(_, state)| **state == ConnectionState::Game)
    else {
        outbox.0.clear();
        return;
    };
    for outgoing in outbox.0.drain(..) {
        match outgoing {
            Outgoing::Click(packet) => connection.write_packet(&packet),
            Outgoing::Close(packet) => connection.write_packet(&packet),
            Outgoing::Action(packet) => connection.write_packet(&packet),
            Outgoing::CreativeSlot(packet) => connection.write_packet(&packet),
        }
    }
}
