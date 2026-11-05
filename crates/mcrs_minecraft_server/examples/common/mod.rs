//! The embedded server and the bot client that the examples share.

use std::net::SocketAddr;
use std::time::Duration;

use bevy_app::{App, Plugin};
use bevy_math::DVec3;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::{GameMode, ServerNetworkPlugin};
use mcrs_minecraft_engine_world::world::lifecycle::simulation::SimulationDistance;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_network::{RawConnection, ReceivedPacket};
use mcrs_minecraft_protocol::anyhow;
use mcrs_minecraft_protocol::packets::common::serverbound::KeepAlive;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundKeepAlive;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet, WritePacket};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundChunkBatchFinished, ClientboundPlayerPosition,
};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundChunkBatchReceived,
};
use mcrs_minecraft_server::MinecraftServerPlugins;

pub const TICK: Duration = Duration::from_millis(50);

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

/// A server on a loopback port that the OS picks, where every player joins in creative mode.
pub fn plugins() -> MinecraftServerPlugins {
    let mut plugins = MinecraftServerPlugins::default();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.network = ServerNetworkPlugin::embedded();
    plugins.network.announce_on_lan = false;
    plugins.worlds.default_game_mode = GameMode::Creative;
    plugins
}

/// Gives every dimension world the simulation distance `horizontal`, when there is one.
pub fn set_simulation_distance(app: &mut App, horizontal: Option<u8>) {
    if let Some(horizontal) = horizontal {
        app.add_dimension_plugin(Simulation(SimulationDistance {
            horizontal,
            vertical: u8::MAX,
        }));
    }
}

#[derive(Clone)]
struct Simulation(SimulationDistance);

impl Plugin for Simulation {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0);
    }
}

/// Joins the server at `address` in offline mode as `username`, with a client that sees
/// `view_distance` columns far.
pub async fn join_bot(
    address: SocketAddr,
    username: String,
    view_distance: u8,
) -> anyhow::Result<RawConnection> {
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
            view_distance,
            holds_vanilla_pack: false,
        },
    )
    .await?;
    Ok(connection)
}

/// What a bot did with a packet that the server sent it.
pub enum Answered {
    /// The server moved the bot to the position; the bot accepted the teleport.
    Teleported(DVec3),
    /// A batch of columns ended; the bot asked for the next one.
    Batch { columns: usize },
    /// The bot answered a keep-alive.
    KeepAlive,
    /// The packet needs no answer.
    Other,
}

/// Answers `packet` as a client must to stay in the game and to keep the columns coming.
pub fn answer(connection: &mut RawConnection, packet: &ReceivedPacket) -> anyhow::Result<Answered> {
    let body = &mut &packet.payload[..];
    if packet.id == ClientboundPlayerPosition::ID {
        let teleport = ClientboundPlayerPosition::decode(body)?;
        connection.write_packet(&ServerboundAcceptTeleportation {
            teleport_id: teleport.teleport_id,
            position: teleport.position.into(),
            look: teleport.look,
        });
        Ok(Answered::Teleported(teleport.position))
    } else if packet.id == ClientboundChunkBatchFinished::ID {
        let finished = ClientboundChunkBatchFinished::decode(body)?;
        connection.write_packet(&ServerboundChunkBatchReceived {
            desired_chunks_per_tick: 64.0,
        });
        let columns = usize::try_from(finished.batch_size.0).unwrap_or(0);
        Ok(Answered::Batch { columns })
    } else if packet.id == ClientboundKeepAlive::ID {
        let keep_alive = ClientboundKeepAlive::decode(body)?;
        connection.write_packet(&ServerboundKeepAlive(KeepAlive {
            payload: keep_alive.0.payload,
        }));
        Ok(Answered::KeepAlive)
    } else {
        Ok(Answered::Other)
    }
}
