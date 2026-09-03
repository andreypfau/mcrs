use crate::event::ReceivedPacketEvent;
use crate::packet_io::{ByteStream, PacketIo};
use crate::{ConnectionState, EngineConnection, RawConnection};
use anyhow::bail;
use bevy_app::{App, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, On, Query};
#[cfg(not(target_family = "wasm"))]
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use log::{error, info, warn};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::serverbound::{ClientInformation, KeepAlive};
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundFinishConfiguration, ClientboundKeepAlive as ConfigurationKeepAlive,
    ClientboundRegistryData, ClientboundSelectKnownPacks, ClientboundUpdateTags,
};
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundFinishConfiguration,
    ServerboundKeepAlive as ServerboundConfigurationKeepAlive, ServerboundSelectKnownPacks,
};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundKeepAlive as GameKeepAlive, ClientboundLogin,
    ClientboundPlayerPosition, ClientboundSetChunkCacheCenter,
};
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive as ServerboundGameKeepAlive;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished, LoginCompression,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::{
    Bounded, CompressionThreshold, Decode, Encode, Look, PROTOCOL_VERSION, Packet, VarInt,
    WritePacket, uuid::Uuid,
};
use md5::{Digest, Md5};
use std::net::SocketAddr;
#[cfg(not(target_family = "wasm"))]
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, channel};

/// The browser has no TCP and the native client has no WebTransport, so what
/// "the server" is differs by target; everything downstream of the byte stream
/// does not.
#[cfg(not(target_family = "wasm"))]
pub type ServerAddress = SocketAddr;
#[cfg(target_family = "wasm")]
pub type ServerAddress = crate::browser::WebTransportTarget;

pub struct ClientNetworkPlugin {
    pub server: ServerAddress,
    pub username: String,
}

/// The socket, its reader and writer tasks, and the outbound encoder.
#[derive(Component)]
pub struct ClientConnection {
    raw: Box<RawConnection>,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ServerProfile {
    pub id: Uuid,
    pub username: String,
}

#[derive(Clone, Debug)]
pub struct RegistryEntry {
    pub id: String,
    pub data: Option<NbtCompound>,
}

#[derive(Clone, Debug)]
pub struct ReceivedRegistry {
    pub registry: String,
    pub entries: Vec<RegistryEntry>,
}

/// Registry snapshots as the server sent them. Nothing reads these yet: which
/// of these and the on-disk assets wins is a separate question.
#[derive(Component, Default, Debug)]
pub struct ReceivedRegistries(pub Vec<ReceivedRegistry>);

#[derive(Clone, Debug)]
pub struct ReceivedTagGroup {
    pub name: String,
    pub entries: Vec<i32>,
}

#[derive(Clone, Debug)]
pub struct ReceivedRegistryTags {
    pub registry: String,
    pub tags: Vec<ReceivedTagGroup>,
}

#[derive(Component, Default, Debug)]
pub struct ReceivedTags(pub Vec<ReceivedRegistryTags>);

#[derive(Component, Clone, Debug)]
pub struct JoinedGame {
    pub player_id: i32,
    pub dimensions: Vec<String>,
    pub dimension: String,
}

#[derive(Clone, Copy, Debug)]
pub struct ServerTeleport {
    pub teleport_id: i32,
    pub position: DVec3,
    pub velocity: DVec3,
    pub look: Look,
}

/// Teleports the server has sent that the player has yet to be moved to and
/// confirm. A queue rather than the latest one, because the server keeps a
/// count of outstanding confirmations and a dropped teleport never unblocks.
#[derive(Component, Default, Debug)]
pub struct PendingTeleports(pub Vec<ServerTeleport>);

#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheCenter {
    pub x: i32,
    pub z: i32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheRadius(pub i32);

#[cfg(not(target_family = "wasm"))]
#[derive(Resource)]
struct ClientRuntime(#[allow(dead_code)] Runtime);

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        let (send, recv) = channel(1);
        let server = self.server.clone();
        let username = self.username.clone();

        let joining = async move {
            match connect_and_log_in(server, username).await {
                Ok(connection) => {
                    let _ = send.send(connection).await;
                }
                Err(e) => error!("login failed: {e:#}"),
            }
        };

        #[cfg(not(target_family = "wasm"))]
        {
            let runtime = Runtime::new().expect("Failed to start the client network runtime");
            runtime.spawn(joining);
            app.insert_resource(ClientRuntime(runtime));
        }
        #[cfg(target_family = "wasm")]
        wasm_bindgen_futures::spawn_local(joining);

        app.add_systems(
            Update,
            (
                spawn_logged_in_connection(recv),
                receive_packets,
                crate::columns::settle_columns,
                flush,
            )
                .chain(),
        );
        app.add_observer(handle_configuration_packet);
        app.add_observer(handle_game_packet);
        crate::columns::build(app);
    }
}

impl WritePacket for ClientConnection {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Encode + Packet,
    {
        self.raw.write_packet_fallible(packet)
    }

    fn write_packet_bytes(&mut self, bytes: &[u8]) {
        self.raw.write_packet_bytes(bytes)
    }
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        let _ = self.raw.flush();
    }
}

/// Vanilla's offline profile id: an MD5 name-based UUID over
/// `OfflinePlayer:<name>`, with no namespace prefix.
pub fn offline_player_uuid(username: &str) -> Uuid {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{username}").as_bytes()).into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

async fn connect_and_log_in(
    server: ServerAddress,
    username: String,
) -> anyhow::Result<(RawConnection, ServerProfile)> {
    #[cfg(not(target_family = "wasm"))]
    {
        let io = PacketIo::connect(server).await?;
        log_in(io, server, server.ip().to_string(), server.port(), username).await
    }
    #[cfg(target_family = "wasm")]
    {
        // A browser never learns the peer address; the connection carries one
        // only so a server-side log line has something to print.
        let peer = SocketAddr::from(([0, 0, 0, 0], 0));
        let (host, port) = server.host_and_port();
        let io = PacketIo::new(crate::browser::connect(&server).await?);
        log_in(io, peer, host, port, username).await
    }
}

/// Handshake and login run before the socket is split in two, because
/// `LoginCompression` changes the framing of every packet after it and the
/// reader task may already have decoded them by the time an ECS system could
/// react.
async fn log_in<S: ByteStream>(
    mut io: PacketIo<S>,
    remote_addr: SocketAddr,
    host: String,
    port: u16,
    username: String,
) -> anyhow::Result<(RawConnection, ServerProfile)> {
    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(PROTOCOL_VERSION),
        server_address: Bounded(host.as_str()),
        server_port: port,
        intent: Intent::Login,
    })
    .await?;
    io.send_packet(&ServerboundHello {
        username: Bounded(username.as_str()),
        profile_id: offline_player_uuid(&username),
    })
    .await?;

    let profile = loop {
        let (id, body) = io.recv_frame().await?;
        let mut r = &body[..];
        if id == ClientboundLoginDisconnect::ID {
            let packet = ClientboundLoginDisconnect::decode(&mut r)?;
            bail!("server refused the login: {}", packet.reason.0);
        } else if id == LoginCompression::ID {
            let packet = LoginCompression::decode(&mut r)?;
            io.set_compression(CompressionThreshold(packet.threshold.0));
        } else if id == ClientboundLoginFinished::ID {
            let packet = ClientboundLoginFinished::decode(&mut r)?;
            break ServerProfile {
                id: packet.profile.id,
                username: packet.profile.username.0.to_owned(),
            };
        } else {
            bail!("unexpected login packet {id:#04x}; only offline login is supported");
        }
    };

    io.send_packet(&ServerboundLoginAcknowledged).await?;
    info!("logged in to {host}:{port} as {}", profile.username);
    Ok((io.into_raw_connection(remote_addr), profile))
}

fn client_information() -> ClientInformation<'static> {
    ClientInformation {
        locale: "en_us",
        view_distance: 8,
        chat_mode: ChatMode::Enabled,
        chat_colors: true,
        displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
        main_arm: MainArm::Right,
        enable_text_filtering: false,
        allow_server_listings: true,
        particle_status: ParticleStatus::All,
    }
}

fn spawn_logged_in_connection(
    mut logged_in: Receiver<(RawConnection, ServerProfile)>,
) -> impl FnMut(&mut World) {
    move |world: &mut World| {
        while let Ok((raw, profile)) = logged_in.try_recv() {
            let mut connection = ClientConnection { raw: Box::new(raw) };
            connection.write_packet(&ServerboundClientInformation(client_information()));
            world.spawn((
                connection,
                ConnectionState::Configuration,
                profile,
                ReceivedRegistries::default(),
                ReceivedTags::default(),
                PendingTeleports::default(),
            ));
        }
    }
}

fn receive_packets(
    mut connections: Query<(Entity, &mut ClientConnection)>,
    mut commands: Commands,
) {
    for (entity, mut connection) in connections.iter_mut() {
        loop {
            match connection.raw.try_recv() {
                Ok(Some(packet)) => commands.trigger(ReceivedPacketEvent {
                    entity,
                    id: packet.id,
                    data: packet.payload,
                    timestamp: packet.timestamp,
                }),
                Ok(None) => break,
                Err(_) => {
                    warn!("the server closed the connection");
                    commands.entity(entity).despawn();
                    break;
                }
            }
        }
    }
}

fn flush(mut connections: Query<&mut ClientConnection>) {
    for mut connection in connections.iter_mut() {
        if let Err(e) = connection.raw.flush() {
            warn!("failed to send to the server: {e}");
        }
    }
}

fn handle_configuration_packet(
    event: On<ReceivedPacketEvent>,
    mut connections: Query<(
        &mut ClientConnection,
        &mut ConnectionState,
        &mut ReceivedRegistries,
        &mut ReceivedTags,
    )>,
) {
    let Ok((mut connection, mut state, mut registries, mut tags)) =
        connections.get_mut(event.entity)
    else {
        return;
    };
    if *state != ConnectionState::Configuration {
        return;
    }

    if event.decode::<ClientboundSelectKnownPacks>().is_some() {
        // The client carries no packs of its own, so every registry entry has
        // to arrive with its data rather than be assumed from a shared pack.
        connection.write_packet(&ServerboundSelectKnownPacks {
            known_packs: Vec::new(),
        });
    } else if let Some(data) = event.decode::<ClientboundRegistryData>() {
        registries.0.push(ReceivedRegistry {
            registry: data.registry.to_string(),
            entries: data
                .entries
                .into_iter()
                .map(|entry| RegistryEntry {
                    id: entry.id.to_string(),
                    data: entry.data.map(|data| data.into_owned()),
                })
                .collect(),
        });
    } else if let Some(update) = event.decode::<ClientboundUpdateTags>() {
        tags.0 = update
            .registries
            .into_iter()
            .map(|registry| ReceivedRegistryTags {
                registry: registry.registry.to_string(),
                tags: registry
                    .tags
                    .into_iter()
                    .map(|group| ReceivedTagGroup {
                        name: group.name.to_string(),
                        entries: group.entries.into_iter().map(|id| id.0).collect(),
                    })
                    .collect(),
            })
            .collect();
    } else if let Some(keep_alive) = event.decode::<ConfigurationKeepAlive>() {
        connection.write_packet(&ServerboundConfigurationKeepAlive(KeepAlive {
            payload: keep_alive.0.payload,
        }));
    } else if event.decode::<ClientboundFinishConfiguration>().is_some() {
        connection.write_packet(&ServerboundFinishConfiguration);
        *state = ConnectionState::Game;
    }
}

fn handle_game_packet(
    event: On<ReceivedPacketEvent>,
    mut connections: Query<(
        &mut ClientConnection,
        &ConnectionState,
        &mut PendingTeleports,
    )>,
    mut commands: Commands,
) {
    let Ok((mut connection, state, mut pending_teleports)) = connections.get_mut(event.entity)
    else {
        return;
    };
    if *state != ConnectionState::Game {
        return;
    }

    if let Some(login) = event.decode::<ClientboundLogin>() {
        commands.entity(event.entity).insert(JoinedGame {
            player_id: login.player_id,
            dimensions: login.dimensions.iter().map(|d| d.to_string()).collect(),
            dimension: login.player_spawn_info.dimension.to_string(),
        });
    } else if let Some(position) = event.decode::<ClientboundPlayerPosition>() {
        if !position.flags.is_empty() {
            // ponytail: every relative flag is treated as absolute. Our server
            // only ever sends absolute teleports; the upgrade is vanilla's
            // `PositionMoveRotation.calculateAbsolute`.
            warn!(
                "relative teleport flags are not applied: {:?}",
                position.flags
            );
        }
        pending_teleports.0.push(ServerTeleport {
            teleport_id: position.teleport_id.0,
            position: position.position,
            velocity: position.velocity,
            look: position.look,
        });
    } else if let Some(center) = event.decode::<ClientboundSetChunkCacheCenter>() {
        commands.entity(event.entity).insert(ChunkCacheCenter {
            x: center.x.0,
            z: center.z.0,
        });
    } else if let Some(radius) = event.decode::<ClientboundChunkCacheRadius>() {
        commands
            .entity(event.entity)
            .insert(ChunkCacheRadius(radius.radius.0));
    } else if let Some(keep_alive) = event.decode::<GameKeepAlive>() {
        connection.write_packet(&ServerboundGameKeepAlive(KeepAlive {
            payload: keep_alive.0.payload,
        }));
    }
}
