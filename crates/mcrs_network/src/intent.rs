use crate::SharedNetworkState;
use crate::packet_io::PacketIo;
use log::debug;
use mcrs_protocol::ConnectionState;
use mcrs_protocol::handshake::Intent;
use mcrs_protocol::packets::intent::serverbound::ServerboundHandshake;
use serde_json::json;

pub(crate) async fn handle_intent(
    shared: SharedNetworkState,
    mut io: PacketIo,
    remote_addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    debug!("Handling intent from {}", remote_addr);
    let (handshake) = io.recv_packet::<ServerboundHandshake>().await?;
    let intent = handshake.intent;

    match intent {
        Intent::Status => {
            let json = json!({
                "version": {
                    "name": "mcrs",
                    "protocol": 773
                },
                "players": {
                    "max": 0,
                    "online": 0,
                    "sample": []
                },
                "description": {
                    "text": "mcrs Server"
                }
            });
            let json_string = json.to_string();
            // let mut listener = ServerStatusPacketListener::new(connection, json_string.as_str());
            // listener.handle().await?;
        }
        Intent::Login => {
            let raw_connection = io.into_raw_connection(remote_addr);
            shared
                .0
                .new_connections_send
                .send(Box::new(raw_connection))
                .await?;
        }
        Intent::Transfer => {}
    }
    Ok(())
}
