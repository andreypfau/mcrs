use crate::event::ReceivedPacketEvent;
use crate::identity;
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
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, VERSION};
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::serverbound::{
    ClientInformation, KeepAlive, ModList, Payload, PropertyMap,
};
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundFinishConfiguration, ClientboundKeepAlive as ConfigurationKeepAlive,
    ClientboundRegistryData, ClientboundSelectKnownPacks, ClientboundUpdateTags,
};
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundCustomPayload, ServerboundFinishConfiguration,
    ServerboundKeepAlive as ServerboundConfigurationKeepAlive, ServerboundSelectKnownPacks,
};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundDisconnect, ClientboundKeepAlive as GameKeepAlive,
    ClientboundLogin, ClientboundPlayerPosition, ClientboundRespawn,
    ClientboundSetChunkCacheCenter,
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
    Bounded, CompressionThreshold, Decode, Encode, Look, Packet, VarInt, WritePacket, uuid::Uuid,
};
use mcrs_minecraft_registry::{LookupIndex, RegistryLookup, RegistrySet};
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
        self.1.declare(&key);
        for (id, entry) in (0..=u16::MAX).zip(&registry.entries) {
            self.1
                .insert(&key, id, ResourceLocation::read(&entry.id).ok());
        }
        self.0.push(registry);
    }
}

impl RegistryLookup for ReceivedRegistries {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u16> {
        self.1.id(registry, name)
    }

    fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation> {
        self.1.name(registry, id)
    }
}

impl ReceivedRegistries {
    /// A registry the server sent, even empty, is numbered by the server alone;
    /// `local` answers only the registries the server never sends.
    pub fn over<'a>(&'a self, local: &'a dyn RegistryLookup) -> ServerNumbering<'a> {
        ServerNumbering {
            received: self,
            local,
        }
    }
}

pub struct ServerNumbering<'a> {
    received: &'a ReceivedRegistries,
    local: &'a dyn RegistryLookup,
}

impl ServerNumbering<'_> {
    fn source(&self, registry: &str) -> &dyn RegistryLookup {
        if self.received.1.holds(registry) {
            self.received
        } else {
            self.local
        }
    }
}

impl RegistryLookup for ServerNumbering<'_> {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u16> {
        self.source(registry).id(registry, name)
    }

    fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation> {
        self.source(registry).name(registry, id)
    }

    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u16> {
        self.local.block_state_id(block, properties)
    }

    fn block_state(&self, id: u16) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        self.local.block_state(id)
    }

    fn registries(&self) -> Option<&RegistrySet> {
        self.local.registries()
    }
}

#[derive(Clone, Debug)]
pub struct ReceivedTagGroup {
    pub name: String,
    pub entries: Vec<u16>,
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
    pub dimensions: Vec<ResourceKey<mcrs_minecraft_dimension::Dimension>>,
    pub dimension: ResourceKey<mcrs_minecraft_dimension::Dimension>,
    /// The server's number for the dimension's type, meaningful only against the registries it sent.
    pub dimension_type_id: u16,
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

/// Closes the connection and ends the session, unless an earlier close in the
/// same frame already did: a disconnect packet is followed by the socket
/// closing, and only the first of the two is the reason.
fn close_connection_later(commands: &mut Commands, connection: Entity, reason: String) {
    commands.queue(move |world: &mut World| {
        if world.despawn(connection) {
            end_session(world, reason);
        }
    });
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
        protocol_version: VarInt(VERSION.protocol_version),
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

fn write_first_configuration_packets(connection: &mut ClientConnection, view_distance: u8) {
    connection.write_packet(&ServerboundCustomPayload::from(Payload::Brand(Brand {
        brand: identity::BRAND,
    })));
    connection.write_packet(&ServerboundCustomPayload::from(Payload::ModList(ModList(
        vec![(
            identity::MOD_ENTRY.into(),
            PropertyMap(vec![(
                identity::COMMIT_PROPERTY.into(),
                identity::COMMIT_HASH,
            )]),
        )],
    ))));
    connection.write_packet(&ServerboundClientInformation(client_information(
        view_distance,
    )));
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
            write_first_configuration_packets(&mut connection, view_distance);
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
                    close_connection_later(
                        &mut commands,
                        entity,
                        "the server closed the connection".into(),
                    );
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
            close_connection_later(
                &mut commands,
                entity,
                "the connection to the server failed".into(),
            );
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
                        entries: group.entries.into_iter().map(u16::from).collect(),
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
        Option<&mut JoinedGame>,
    )>,
    mut commands: Commands,
) {
    let Ok((mut connection, state, mut pending_teleports, joined)) =
        connections.get_mut(event.entity)
    else {
        return;
    };
    if *state != ConnectionState::Game {
        return;
    }

    if let Some(disconnect) = event.decode::<ClientboundDisconnect>() {
        close_connection_later(
            &mut commands,
            event.entity,
            format!(
                "the server disconnected you: {}",
                disconnect.reason.to_legacy_lossy()
            ),
        );
    } else if let Some(login) = event.decode::<ClientboundLogin>() {
        commands.entity(event.entity).insert(JoinedGame {
            player_id: login.player_id,
            dimensions: login.dimensions,
            dimension: login.player_spawn_info.dimension,
            dimension_type_id: login.player_spawn_info.dimension_type_id.0,
        });
    } else if let Some(respawn) = event.decode::<ClientboundRespawn>() {
        if let Some(mut joined) = joined {
            joined.dimension = respawn.player_spawn_info.dimension;
            joined.dimension_type_id = respawn.player_spawn_info.dimension_type_id.0;
        }
    } else if let Some(position) = event.decode::<ClientboundPlayerPosition>() {
        if !position.flags.is_empty() {
            // chisle: every relative flag is treated as absolute. Our server
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
    fn a_closed_connection_ends_the_session() {
        let runtime = Runtime::new().unwrap();
        let (mut app, inbound, _) = client_app(&runtime);
        app.update();
        assert!(app.should_exit().is_none());
        drop(inbound);
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
    }

    #[test]
    fn a_disconnect_packet_ends_the_session() {
        let runtime = Runtime::new().unwrap();
        let (mut app, inbound, entity) = client_app(&runtime);
        deliver(
            &runtime,
            &inbound,
            &ClientboundDisconnect {
                reason: Text::text("the server is restarting"),
            },
        );
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
        assert!(app.world().get_entity(entity).is_err());
    }

    #[test]
    fn the_client_sends_brand_then_mod_list_then_client_information() {
        use crate::identity;
        use bytes::BytesMut;
        use mcrs_minecraft_protocol::decode::PacketDecoder;
        use mcrs_minecraft_protocol::packets::common::serverbound::{ModList, Payload};
        use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundCustomPayload;

        let runtime = Runtime::new().unwrap();
        let _guard = runtime.enter();
        let (raw, mut outgoing, _inbound) = RawConnection::new_for_test_full(8);
        let mut connection = ClientConnection { raw: Box::new(raw) };

        write_first_configuration_packets(&mut connection, 11);
        connection.raw.flush().unwrap();

        let mut decoder = PacketDecoder::new();
        while let Ok(blob) = outgoing.try_recv() {
            decoder.queue_bytes(BytesMut::from(&blob[..]));
        }
        let frames: Vec<_> = std::iter::from_fn(|| decoder.try_next_packet().unwrap()).collect();

        let ids: Vec<i32> = frames.iter().map(|frame| frame.id).collect();
        assert_eq!(
            ids,
            [
                ServerboundCustomPayload::ID,
                ServerboundCustomPayload::ID,
                ServerboundClientInformation::ID
            ]
        );

        let Payload::Brand(brand) = Payload::decode(&mut &frames[0].body[..]).unwrap() else {
            panic!("the first payload is not a brand");
        };
        assert_eq!(brand.brand, identity::BRAND);

        let Payload::ModList(ModList(entries)) = Payload::decode(&mut &frames[1].body[..]).unwrap()
        else {
            panic!("the second payload is not a mod list");
        };
        assert_eq!(entries.len(), 1);
        let (id, properties) = &entries[0];
        assert_eq!(*id, identity::MOD_ENTRY);
        assert_eq!(properties.0.len(), 1);
        assert_eq!(properties.0[0].0, identity::COMMIT_PROPERTY);
        assert_eq!(properties.0[0].1, identity::COMMIT_HASH);

        let information = ClientInformation::decode(&mut &frames[2].body[..]).unwrap();
        assert_eq!(information.view_distance, 11);
    }

    fn deliver<P: Encode + Packet>(
        runtime: &Runtime,
        inbound: &mpsc::Sender<crate::ReceivedPacket>,
        packet: &P,
    ) {
        let mut body = Vec::new();
        packet.encode(&mut body).unwrap();
        runtime
            .block_on(inbound.send(crate::ReceivedPacket {
                timestamp: crate::Instant::now(),
                id: P::ID,
                payload: body.into(),
            }))
            .unwrap();
    }

    #[test]
    fn a_respawn_moves_the_joined_game_to_the_dimension_and_type_it_names() {
        use mcrs_minecraft_protocol::RegistryId;
        use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
        use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundRespawn;

        let runtime = Runtime::new().unwrap();
        let (mut app, inbound, entity) = client_app(&runtime);
        let spawn_in = |dimension: ResourceKey<mcrs_minecraft_dimension::Dimension>,
                        type_id: u16| PlayerSpawnInfo {
            dimension,
            dimension_type_id: RegistryId(type_id),
            ..Default::default()
        };

        deliver(
            &runtime,
            &inbound,
            &ClientboundLogin {
                player_id: 7,
                hardcore: false,
                dimensions: vec![mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into()],
                max_players: VarInt(1),
                chunk_radius: VarInt(8),
                simulation_distance: VarInt(8),
                reduced_debug_info: false,
                show_death_screen: true,
                do_limited_crafting: false,
                player_spawn_info: spawn_in(
                    mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
                    3,
                ),
                online_mode: false,
                enforces_secure_chat: false,
            },
        );
        app.update();
        let joined = app.world().get::<JoinedGame>(entity).unwrap();
        assert_eq!(
            joined.dimension,
            mcrs_minecraft_dimension::keys::dimension::OVERWORLD
        );
        assert_eq!(joined.dimension_type_id, 3);

        deliver(
            &runtime,
            &inbound,
            &ClientboundRespawn {
                player_spawn_info: spawn_in(
                    mcrs_minecraft_dimension::keys::dimension::THE_NETHER.into(),
                    1,
                ),
                data_to_keep: 0,
            },
        );
        app.update();
        let joined = app.world().get::<JoinedGame>(entity).unwrap();
        assert_eq!(joined.player_id, 7);
        assert_eq!(
            joined.dimension,
            mcrs_minecraft_dimension::keys::dimension::THE_NETHER
        );
        assert_eq!(joined.dimension_type_id, 1);
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
    use mcrs_minecraft_core::rl;

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
        let unbreaking = rl!("minecraft:unbreaking").to_arc();
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
            Some(&rl!("minecraft:lava").to_arc())
        );
    }

    #[test]
    fn a_registry_the_server_sent_hides_the_local_numbering_even_when_empty() {
        let entry = |id: &str| RegistryEntry {
            id: id.to_owned(),
            data: None,
        };
        let mut local = ReceivedRegistries::default();
        for registry in ["minecraft:enchantment", "minecraft:item"] {
            local.push(ReceivedRegistry {
                registry: registry.to_owned(),
                entries: vec![entry("minecraft:a")],
            });
        }
        let mut server = ReceivedRegistries::default();
        server.push(ReceivedRegistry {
            registry: "minecraft:enchantment".to_owned(),
            entries: Vec::new(),
        });
        let lookup = server.over(&local);
        let a = rl!("minecraft:a").to_arc();
        assert_eq!(lookup.name("enchantment", 0), None);
        assert_eq!(lookup.id("enchantment", &a), None);
        assert_eq!(lookup.name("item", 0), Some(&a));
        assert_eq!(lookup.id("item", &a), Some(0));
    }
}
