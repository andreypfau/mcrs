use crate::packet_io::{ByteStream, PacketIo};
use crate::{SharedNetworkState, SwitchMode};
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::packets::ping::clientbound::PongResponse;
use mcrs_minecraft_protocol::packets::ping::serverbound::PingRequest;
use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Text};
use rand::seq::SliceRandom;
use serde::Serialize;
use std::time::Duration;
use tokio::time::timeout;
use tracing::debug;

/// The game calls a client older than protocol 754 outdated, and names its own version in
/// either reason.
fn unsupported_version_reason(protocol_version: i32) -> Text {
    let key = if protocol_version < 754 {
        "multiplayer.disconnect.outdated_client"
    } else {
        "multiplayer.disconnect.incompatible"
    };
    Text::translate(key, vec![Text::from(VERSION.name)])
}

#[derive(Serialize)]
struct Status {
    version: Version,
    players: Players,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Text>,
}

#[derive(Serialize)]
struct Version {
    name: String,
    protocol: i32,
}

#[derive(Serialize)]
struct Sample {
    id: Uuid,
    name: String,
}

#[derive(Serialize)]
struct Players {
    max: u32,
    online: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sample: Vec<Sample>,
}

const REFUSAL_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

async fn refuse_login<S: ByteStream>(mut io: PacketIo<S>, reason: &Text) -> anyhow::Result<()> {
    let reason = serde_json::to_string(reason)?;
    io.send_packet(&ClientboundLoginDisconnect {
        reason: Bounded(&reason),
    })
    .await?;
    // A client that never closes only costs the timeout; the reason is already sent.
    let _ = timeout(REFUSAL_CLOSE_TIMEOUT, io.close()).await;
    Ok(())
}

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
            if !shared.0.enable_status {
                return Ok(());
            }
            let _request = io
                .recv_packet::<mcrs_minecraft_protocol::packets::status::serverbound::StatusRequest>()
                .await?;

            let json = {
                let players = shared.0.status_recv.borrow();
                let mut samples = Vec::new();

                if !shared.0.hide_online_players {
                    let n = players.players.len();
                    let start = if n > 12 {
                        rand::random_range(0..=n - 12)
                    } else {
                        0
                    };
                    for (uuid, name) in players.players.iter().skip(start).take(12) {
                        samples.push(Sample {
                            id: *uuid,
                            name: name.clone(),
                        });
                    }
                    samples.shuffle(&mut rand::rng());
                }

                let status = Status {
                    version: Version {
                        name: VERSION.name.to_owned(),
                        protocol: VERSION.protocol_version,
                    },
                    players: Players {
                        max: shared.0.max_players,
                        online: u32::try_from(players.players.len()).unwrap_or(u32::MAX),
                        sample: samples,
                    },
                    description: (!shared.0.motd.is_empty())
                        .then(|| Text::from(shared.0.motd.clone())),
                };
                serde_json::to_string(&status)?
            };

            io.send_packet(&StatusResponse { json: &json }).await?;

            if let Ok(ping) = io.recv_packet::<PingRequest>().await {
                io.send_packet(&PongResponse {
                    payload: ping.payload,
                })
                .await?;
            }
        }
        Intent::Transfer if !shared.0.accepts_transfers => {
            let reason = Text::translate("multiplayer.disconnect.transfers_disabled", vec![]);
            refuse_login(io, &reason).await?;
        }
        Intent::Login | Intent::Transfer => {
            let Some(switch_mode) = SwitchMode::of(protocol_version) else {
                return refuse_login(io, &unsupported_version_reason(protocol_version)).await;
            };
            let raw_connection = io.into_raw_connection(remote_addr, switch_mode);
            shared
                .0
                .new_connections_send
                .send(Box::new(raw_connection))
                .await?;
        }
    }
    Ok(())
}
