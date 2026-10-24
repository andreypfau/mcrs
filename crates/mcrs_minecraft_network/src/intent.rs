use crate::packet_io::{ByteStream, PacketIo};
use crate::{SharedNetworkState, SwitchMode};
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::packets::ping::clientbound::PongResponse;
use mcrs_minecraft_protocol::packets::ping::serverbound::PingRequest;
use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
use mcrs_minecraft_protocol::{Bounded, Text};
use serde_json::json;
use tracing::debug;

/// The game calls a client older than protocol 754 outdated, and names its own version in
/// either reason.
fn unsupported_version_reason(protocol_version: i32) -> Text {
    let key = if protocol_version < 754 {
        "multiplayer.disconnect.outdated_client"
    } else {
        "multiplayer.disconnect.incompatible"
    };
    Text::translate(key, vec![Text::from(VERSION.name.clone())])
}

pub(crate) const MOTD: &str = "mcrs Server";

#[expect(
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "the status response is still written as a JSON literal; its typed form also needs the player count from the engine"
)]
pub(crate) async fn handle_intent<S: ByteStream>(
    shared: SharedNetworkState,
    mut io: PacketIo<S>,
    remote_addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    debug!("Handling intent from {}", remote_addr);
    let handshake = io.recv_packet::<ServerboundHandshake>().await?;
    let intent = handshake.intent;
    let protocol_version = handshake.protocol_version.0;

    match intent {
        Intent::Status => {
            let _request = io
                .recv_packet::<mcrs_minecraft_protocol::packets::status::serverbound::StatusRequest>()
                .await?;
            let json = json!({
                "version": {
                    "name": "mcrs",
                    "protocol": VERSION.protocol_version
                },
                "players": {
                    "max": 0,
                    "online": 0,
                    "sample": []
                },
                "description": {
                    "text": MOTD
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
            let Some(switch_mode) = SwitchMode::of(protocol_version) else {
                let reason = serde_json::to_string(&unsupported_version_reason(protocol_version))?;
                io.send_packet(&ClientboundLoginDisconnect {
                    reason: Bounded(&reason),
                })
                .await?;
                return Ok(());
            };
            let raw_connection = io.into_raw_connection(remote_addr, switch_mode);
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
