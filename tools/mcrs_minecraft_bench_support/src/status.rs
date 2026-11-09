use std::net::SocketAddr;
use std::time::Duration;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
use mcrs_minecraft_protocol::packets::status::serverbound::StatusRequest;
use mcrs_minecraft_protocol::{Bounded, VarInt};

/// Whether the server at `address` answers a status request, which every server does once its
/// tick loop runs. A server of another protocol version answers too.
pub async fn answers(address: SocketAddr) -> bool {
    let request = async {
        let mut io = PacketIo::connect(address).await?;
        io.send_packet(&ServerboundHandshake {
            protocol_version: VarInt(VERSION.protocol_version),
            server_address: Bounded("127.0.0.1"),
            server_port: address.port(),
            intent: Intent::Status,
        })
        .await?;
        io.send_packet(&StatusRequest).await?;
        io.recv_packet::<StatusResponse>().await?;
        anyhow::Ok(())
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(2), request).await,
        Ok(Ok(()))
    )
}
