use bevy::prelude::*;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::serverbound::{ClientCommandAction, ServerboundClientCommand};

#[derive(Message, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequestRespawn;

pub(crate) fn send_respawn_requests(
    mut requests: MessageReader<RequestRespawn>,
    connection: Option<Single<&mut ClientConnection>>,
) {
    let Some(mut connection) = connection else {
        requests.clear();
        return;
    };
    for RequestRespawn in requests.read() {
        connection.write_packet(&ServerboundClientCommand {
            action: ClientCommandAction::PerformRespawn,
        });
    }
}
