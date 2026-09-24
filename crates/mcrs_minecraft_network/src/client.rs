use crate::event::ReceivedPacketEvent;
use crate::packet_io::{ByteStream, PacketIo};
use crate::{ConnectionState, RawConnection};
use anyhow::bail;
use bevy_app::{App, AppExit, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, On, Query};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::ColumnPos;
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
    ClientboundChunkCacheRadius, ClientboundDisconnect, ClientboundKeepAlive as GameKeepAlive,
    ClientboundLogin, ClientboundPlayerPosition, ClientboundSetChunkCacheCenter,
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
use mcrs_minecraft_registry::{LookupIndex, RegistryLookup};
use md5::{Digest, Md5};
use std::net::SocketAddr;
#[cfg(not(target_family = "wasm"))]
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, channel};
use tracing::{error, info, warn};

/// The browser has no TCP and the native client has no WebTransport, so what
/// "the server" is differs by target; everything downstream of the byte stream
/// does not.
#[cfg(not(target_family = "wasm"))]
pub type ServerAddress = SocketAddr;
#[cfg(target_family = "wasm")]
pub type ServerAddress = crate::browser::WebTransportTarget;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientNetworkSystems {
    Receive,
    Flush,
}

pub struct ClientNetworkPlugin {
    pub server: ServerAddress,
    pub username: String,
    pub profile_id: Option<Uuid>,
    pub view_distance: u8,
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
    pub data: Option<mcrs_minecraft_nbt::tag::NbtTag>,
}

#[derive(Clone, Debug)]
pub struct ReceivedRegistry {
    pub registry: String,
    pub entries: Vec<RegistryEntry>,
}

/// Registry snapshots as the server sent them, in network id order. Which of
/// these and the on-disk assets wins for game data is a separate question;
/// as the `RegistryLookup` for stacks they are the only authority.
#[derive(Component, Default, Debug)]
pub struct ReceivedRegistries(pub Vec<ReceivedRegistry>, LookupIndex);

impl ReceivedRegistries {
    pub fn push(&mut self, registry: ReceivedRegistry) {
        let key: Box<str> = registry
            .registry
            .split_once(':')
            .map_or(registry.registry.as_str(), |(_, path)| path)
            .into();
        for (id, entry) in registry.entries.iter().enumerate() {
            self.1
                .insert(&key, id as u32, ResourceLocation::parse(&entry.id).ok());
        }
        self.0.push(registry);
    }
}

impl RegistryLookup for ReceivedRegistries {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.1.id(registry, name)
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.1.name(registry, id)
    }
}

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
pub struct ChunkCacheCenter(pub ColumnPos);

#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheRadius(pub i32);

#[cfg(not(target_family = "wasm"))]
#[derive(Resource)]
struct ClientRuntime(#[allow(dead_code)] Runtime);

/// Without it a lost connection only logs: a test harness or a browser tab
/// outlives its connection, a windowed client has nothing left to show.
#[derive(Resource)]
pub struct ExitOnDisconnect;

fn end_session(world: &mut World, reason: String) {
    error!("{reason}");
    if world.contains_resource::<ExitOnDisconnect>() {
        world.write_message(AppExit::Success);
    }
}

fn end_session_later(commands: &mut Commands, reason: String) {
    commands.queue(move |world: &mut World| end_session(world, reason));
}

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        let (send, recv) = channel(1);
        let server = self.server.clone();
        let username = self.username.clone();
        let profile_id = self
            .profile_id
            .unwrap_or_else(|| offline_player_uuid(&username));
        let view_distance = self.view_distance;

        let joining = async move {
            let outcome = connect_and_log_in(server, username, profile_id)
                .await
                .map_err(|e| format!("{e:#}"));
            let _ = send.send(outcome).await;
        };

        #[cfg(not(target_family = "wasm"))]
        {
            let runtime = Runtime::new().expect("Failed to start the client network runtime");
            runtime.spawn(joining);
            app.insert_resource(ClientRuntime(runtime));
        }
        #[cfg(target_family = "wasm")]
        wasm_bindgen_futures::spawn_local(joining);

        app.configure_sets(
            Update,
            (ClientNetworkSystems::Receive, ClientNetworkSystems::Flush).chain(),
        );
        app.add_systems(
            Update,
            (
                spawn_logged_in_connection(recv, view_distance),
                receive_packets,
            )
                .chain()
                .in_set(ClientNetworkSystems::Receive),
        );
        app.add_systems(Update, flush.in_set(ClientNetworkSystems::Flush));
        app.add_observer(handle_configuration_packet);
        app.add_observer(handle_game_packet);
    }
}

impl WritePacket for ClientConnection {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Encode + Packet,
    {
        self.raw.write_packet_fallible(packet)
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
    mcrs_minecraft_protocol::uuid::Builder::from_md5_bytes(
        Md5::digest(format!("OfflinePlayer:{username}").as_bytes()).into(),
    )
    .into_uuid()
}

async fn connect_and_log_in(
    server: ServerAddress,
    username: String,
    profile_id: Uuid,
) -> anyhow::Result<(RawConnection, ServerProfile)> {
    #[cfg(not(target_family = "wasm"))]
    {
        let io = PacketIo::connect(server).await?;
        log_in(
            io,
            server,
            server.ip().to_string(),
            server.port(),
            username,
            profile_id,
        )
        .await
    }
    #[cfg(target_family = "wasm")]
    {
        // A browser never learns the peer address; the connection carries one
        // only so a server-side log line has something to print.
        let peer = SocketAddr::from(([0, 0, 0, 0], 0));
        let (host, port) = server.host_and_port();
        let io = PacketIo::new(crate::browser::connect(&server).await?);
        log_in(io, peer, host, port, username, profile_id).await
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
    profile_id: Uuid,
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
        profile_id,
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

fn client_information(view_distance: u8) -> ClientInformation<'static> {
    ClientInformation {
        locale: "en_us",
        view_distance,
        chat_mode: ChatMode::Enabled,
        chat_colors: true,
        displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
        main_arm: MainArm::Right,
        enable_text_filtering: false,
        allow_server_listings: true,
        particle_status: ParticleStatus::All,
    }
}

type LoginOutcome = Result<(RawConnection, ServerProfile), String>;

fn spawn_logged_in_connection(
    mut logged_in: Receiver<LoginOutcome>,
    view_distance: u8,
) -> impl FnMut(&mut World) {
    move |world: &mut World| {
        while let Ok(outcome) = logged_in.try_recv() {
            let (raw, profile) = match outcome {
                Ok(connection) => connection,
                Err(e) => {
                    end_session(world, format!("the connection failed: {e}"));
                    continue;
                }
            };
            let mut connection = ClientConnection { raw: Box::new(raw) };
            connection.write_packet(&ServerboundClientInformation(client_information(
                view_distance,
            )));
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
                    commands.entity(entity).despawn();
                    end_session_later(&mut commands, "the server closed the connection".into());
                    break;
                }
            }
        }
    }
}

fn flush(mut connections: Query<(Entity, &mut ClientConnection)>, mut commands: Commands) {
    for (entity, mut connection) in connections.iter_mut() {
        if let Err(e) = connection.raw.flush() {
            warn!("failed to send to the server: {e}");
        }
        // The flush error above is also raised by a writer that is merely
        // behind, so the dead writer is what the disconnect is read from.
        if connection.raw.disconnected() {
            commands.entity(entity).despawn();
            end_session_later(&mut commands, "the connection to the server failed".into());
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
        registries.push(ReceivedRegistry {
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

    if let Some(disconnect) = event.decode::<ClientboundDisconnect>() {
        commands.entity(event.entity).despawn();
        end_session_later(
            &mut commands,
            format!(
                "the server disconnected you: {}",
                disconnect.reason.to_legacy_lossy()
            ),
        );
    } else if let Some(login) = event.decode::<ClientboundLogin>() {
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
        commands
            .entity(event.entity)
            .insert(ChunkCacheCenter(ColumnPos::new(center.x.0, center.z.0)));
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

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use bevy_app::Update;
    use mcrs_minecraft_protocol::text::Text;
    use tokio::sync::mpsc;

    fn client_app(runtime: &Runtime) -> (App, mpsc::Sender<crate::ReceivedPacket>, Entity) {
        let _guard = runtime.enter();
        let (raw, _outgoing, inbound) = RawConnection::new_for_test_full(8);
        let mut app = App::new();
        app.insert_resource(ExitOnDisconnect);
        app.add_systems(Update, (receive_packets, flush).chain());
        app.add_observer(handle_game_packet);
        let entity = app
            .world_mut()
            .spawn((
                ClientConnection { raw: Box::new(raw) },
                ConnectionState::Game,
                PendingTeleports::default(),
            ))
            .id();
        (app, inbound, entity)
    }

    #[test]
    fn a_live_connection_does_not_end_the_session() {
        let runtime = Runtime::new().unwrap();
        let (mut app, _inbound, _) = client_app(&runtime);
        app.update();
        assert!(app.should_exit().is_none());
    }

    #[test]
    fn a_closed_connection_ends_the_session() {
        let runtime = Runtime::new().unwrap();
        let (mut app, inbound, _) = client_app(&runtime);
        app.update();
        drop(inbound);
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
    }

    #[test]
    fn a_disconnect_packet_ends_the_session() {
        let runtime = Runtime::new().unwrap();
        let (mut app, inbound, entity) = client_app(&runtime);
        let mut body = Vec::new();
        ClientboundDisconnect {
            reason: Text::text("the server is restarting"),
        }
        .encode(&mut body)
        .unwrap();
        runtime
            .block_on(inbound.send(crate::ReceivedPacket {
                timestamp: crate::Instant::now(),
                id: ClientboundDisconnect::ID,
                payload: body.into(),
            }))
            .unwrap();
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
        assert!(app.world().get_entity(entity).is_err());
    }

    #[test]
    fn without_the_resource_a_closed_connection_only_logs() {
        let runtime = Runtime::new().unwrap();
        let (mut app, inbound, _) = client_app(&runtime);
        app.world_mut().remove_resource::<ExitOnDisconnect>();
        drop(inbound);
        app.update();
        assert!(app.should_exit().is_none());
    }
}

#[cfg(test)]
mod lookup_tests {
    use super::*;

    #[test]
    fn offline_player_uuid_matches_vanilla() {
        assert_eq!(
            offline_player_uuid("Notch").to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn received_registries_resolve_names_and_network_ids() {
        let mut registries = ReceivedRegistries::default();
        let entry = |id: &str| RegistryEntry {
            id: id.to_owned(),
            data: None,
        };
        registries.push(ReceivedRegistry {
            registry: "minecraft:enchantment".to_owned(),
            entries: vec![entry("minecraft:sharpness"), entry("minecraft:unbreaking")],
        });
        let unbreaking = ResourceLocation::minecraft("unbreaking");
        assert_eq!(registries.id("enchantment", &unbreaking), Some(1));
        assert_eq!(registries.name("enchantment", 1), Some(&unbreaking));
        assert_eq!(registries.name("enchantment", 2), None);
        assert_eq!(registries.id("item", &unbreaking), None);

        registries.push(ReceivedRegistry {
            registry: "minecraft:damage_type".to_owned(),
            entries: vec![entry("minecraft:lava")],
        });
        assert_eq!(
            registries.name("damage_type", 0),
            Some(&ResourceLocation::minecraft("lava"))
        );
    }
}
