//! The upkeep a headless protocol client owes the server so that it keeps the client in the world.

use std::net::SocketAddr;
use std::time::Duration;

use glam::DVec3;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::RawConnection;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::packets::common::serverbound::{KeepAlive, Pong};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundKeepAlive;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet, WritePacket};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundChunkBatchFinished, ClientboundPing, ClientboundPlayerPosition,
};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundChunkBatchReceived, ServerboundPlayerAbilities,
    ServerboundPlayerLoaded, ServerboundPong,
};

/// The length of a client tick.
pub const TICK: Duration = Duration::from_millis(50);
pub const VIEW_DISTANCE: u8 = 10;
/// Both servers clamp the asked rate to their own ceiling, so asking for the most leaves the
/// pace to the server's batch window, the same rule on both.
pub const DESIRED_CHUNKS_PER_TICK: f32 = 64.0;

/// What [`answer_upkeep`] did with a packet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Upkeep {
    /// The packet was an upkeep request and is answered.
    Handled,
    /// The server placed the player at this position; the teleport is accepted.
    Teleported(DVec3),
    /// The packet is none of the upkeep packets.
    Other,
}

/// Joins the server at `address` as `username` and returns the play connection.
pub async fn connect(address: SocketAddr, username: String) -> anyhow::Result<RawConnection> {
    let io = PacketIo::connect(address).await?;
    let Joined { connection, .. } = join(
        io,
        Join {
            server_address: address.ip().to_string(),
            server_port: address.port(),
            remote_addr: address,
            username,
            profile_id: Uuid::nil(),
            protocol_version: VERSION.protocol_version,
            view_distance: VIEW_DISTANCE,
            holds_vanilla_pack: false,
        },
    )
    .await?;
    Ok(connection)
}

/// Answers the chunk batch, keep-alive, ping and teleport packets, which every client answers.
pub fn answer_upkeep(
    connection: &mut RawConnection,
    id: i32,
    body: &mut &[u8],
) -> anyhow::Result<Upkeep> {
    if id == ClientboundChunkBatchFinished::ID {
        connection.write_packet(&ServerboundChunkBatchReceived {
            desired_chunks_per_tick: DESIRED_CHUNKS_PER_TICK,
        });
    } else if id == ClientboundKeepAlive::ID {
        let keep_alive = ClientboundKeepAlive::decode(body)?;
        connection.write_packet(&ServerboundKeepAlive(KeepAlive {
            payload: keep_alive.0.payload,
        }));
    } else if id == ClientboundPing::ID {
        let ping = ClientboundPing::decode(body)?;
        connection.write_packet(&ServerboundPong(Pong { payload: ping.id }));
    } else if id == ClientboundPlayerPosition::ID {
        let teleport = ClientboundPlayerPosition::decode(body)?;
        connection.write_packet(&ServerboundAcceptTeleportation {
            teleport_id: teleport.teleport_id,
            position: teleport.position.into(),
            look: teleport.look,
        });
        return Ok(Upkeep::Teleported(teleport.position));
    } else {
        return Ok(Upkeep::Other);
    }
    Ok(Upkeep::Handled)
}

/// Tells the server that the client is in the world and flies; sent after the first teleport.
pub fn announce_loaded(connection: &mut RawConnection) {
    connection.write_packet(&ServerboundPlayerLoaded);
    connection.write_packet(&ServerboundPlayerAbilities { flying: true });
}
