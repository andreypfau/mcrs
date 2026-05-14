use crate::SharedNetworkState;
use crate::packet_io::PacketIo;
use log::debug;
use mcrs_protocol::PROTOCOL_VERSION;
use mcrs_protocol::handshake::Intent;
use mcrs_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_protocol::packets::ping::clientbound::PongResponse;
use mcrs_protocol::packets::ping::serverbound::PingRequest;
use mcrs_protocol::packets::status::clientbound::StatusResponse;
use serde_json::json;

pub(crate) async fn handle_intent(
    shared: SharedNetworkState,
    mut io: PacketIo,
    remote_addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    debug!("Handling intent from {}", remote_addr);
    let handshake = io.recv_packet::<ServerboundHandshake>().await?;
    let intent = handshake.intent;

    match intent {
        Intent::Status => {
            let _request = io
                .recv_packet::<mcrs_protocol::packets::status::serverbound::StatusRequest>()
                .await?;
            let json = json!({
                "version": {
                    "name": "mcrs",
                    "protocol": PROTOCOL_VERSION
                },
                "players": {
                    "max": 0,
                    "online": 0,
                    "sample": []
                },
                "description": {
                    "text": "mcrs Server"
                }
            })
            .to_string();
            io.send_packet(&StatusResponse { json: &json }).await?;

            if let Ok(ping) = io.recv_packet::<PingRequest>().await {
                io.send_packet(&PongResponse {
                    payload: ping.payload,
                })
                .await?;
            }
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
