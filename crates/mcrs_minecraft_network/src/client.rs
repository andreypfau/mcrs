use crate::event::ReceivedPacketEvent;
use crate::identity;
use crate::packet_io::{ByteStream, PacketIo};
use crate::{ConnectionState, RawConnection, SwitchMode};
use anyhow::bail;
use bevy_app::{App, AppExit, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::prelude::{Commands, On, Query, Res};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::{ResourceKey, VERSION};
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::serverbound::{
    ClientInformation, KeepAlive, ModList, Payload, PropertyMap,
};
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundFinishConfiguration, ClientboundKeepAlive as ConfigurationKeepAlive,
    ClientboundRegistryData, ClientboundSelectKnownPacks, ClientboundUpdateTags, RegistryTags,
};
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundCustomPayload, ServerboundFinishConfiguration,
    ServerboundKeepAlive as ServerboundConfigurationKeepAlive, ServerboundSelectKnownPacks,
};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundDisconnect, ClientboundKeepAlive as GameKeepAlive,
    ClientboundLogin, ClientboundPlayerPosition, ClientboundRespawn,
    ClientboundSetChunkCacheCenter, ClientboundSetEntityData, ClientboundStartConfiguration,
    ClientboundUpdateTags as ClientboundGameUpdateTags,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundConfigurationAcknowledged, ServerboundKeepAlive as ServerboundGameKeepAlive,
};
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished, ClientboundLoginFinishedWithOptions,
    LoginCompression, OptionFlags,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::{
    Bounded, CompressionThreshold, Decode, Encode, Look, Packet, VarInt, WritePacket, uuid::Uuid,
};
use mcrs_minecraft_registries::{CurrentRegistries, Namer, Received, ReceivedTags};
use md5::{Digest, Md5};
use std::borrow::Cow;
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

/// Present when the client holds the vanilla pack of its own version, so it may let a server
/// leave that pack's entries out.
#[derive(Resource)]
pub struct HoldsVanillaPack;

fn collect_registry(received: &mut Received, data: ClientboundRegistryData<'_>) {
    let registry: ResourceLocation = data.registry.into();
    let entries = data
        .entries
        .into_iter()
        .map(|entry| (entry.id.into(), entry.data.map(Cow::into_owned)));
    match received
        .registries
        .iter_mut()
        .find(|(collected, _)| *collected == registry)
    {
        Some((_, collected)) => collected.extend(entries),
        None => received.registries.push((registry, entries.collect())),
    }
}

fn received_tags(update: Vec<RegistryTags<'_>>) -> ReceivedTags {
    update
        .into_iter()
        .map(|registry| {
            let tags = registry
                .tags
                .into_iter()
                .map(|group| {
                    let members = group.entries.into_iter().map(|member| member.0).collect();
                    (group.name.into(), members)
                })
                .collect();
            (registry.registry.into(), tags)
        })
        .collect()
}

#[derive(Component, Default)]
struct ConfigurationPackets {
    received: Received,
    vanilla_known: bool,
}

/// A server ended its configuration. The connection stays in configuration until an observer
/// answers with [`finish_configuration`] or drops it with [`close_connection_later`].
#[derive(EntityEvent, Debug)]
pub struct RegistriesReceived {
    pub entity: Entity,
    pub registries: Received,
    /// The client accepted the vanilla pack, so the server may leave its entries' data out.
    pub vanilla_known: bool,
}

/// A server replaced the tags of the registries it names during play.
#[derive(EntityEvent, Debug)]
pub struct TagsReceived {
    pub entity: Entity,
    pub tags: ReceivedTags,
}

pub fn finish_configuration(commands: &mut Commands, connection: Entity) {
    commands.queue(move |world: &mut World| {
        let Ok(mut entity) = world.get_entity_mut(connection) else {
            return;
        };
        if let Some(mut client) = entity.get_mut::<ClientConnection>() {
            client.write_packet(&ServerboundFinishConfiguration);
        }
        if let Some(mut state) = entity.get_mut::<ConnectionState>() {
            *state = ConnectionState::Game;
        }
    });
}

#[derive(Component, Clone, Debug)]
pub struct JoinedGame {
    pub player_id: i32,
    pub dimensions: Vec<ResourceKey<Dimension>>,
}

/// The dimension the player is in. Written only when a login or a respawn
/// names it, as a whole.
#[derive(Component, Clone, Debug)]
pub struct CurrentDimension {
    pub key: ResourceKey<Dimension>,
    pub dimension_type: Id<DimensionType>,
}

impl CurrentDimension {
    pub fn of(spawn: &PlayerSpawnInfo) -> Self {
        CurrentDimension {
            key: spawn.dimension.clone(),
            dimension_type: spawn.dimension_type_id,
        }
    }
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
struct ClientRuntime(
    #[expect(
        dead_code,
        reason = "the runtime is held only so its tasks live as long as the process, and nothing reads it"
    )]
    Runtime,
);

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
pub fn close_connection_later(commands: &mut Commands, connection: Entity, reason: String) {
    commands.queue(move |world: &mut World| {
        if world.despawn(connection) {
            end_session(world, reason);
        }
    });
}

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        let (send, recv) = channel(1);
        #[cfg_attr(
            not(target_family = "wasm"),
            expect(
                clippy::clone_on_copy,
                reason = "ServerAddress is Copy only on native; the browser's WebTransportTarget is not"
            )
        )]
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
            VERSION.protocol_version,
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
        log_in(
            io,
            peer,
            host,
            port,
            username,
            profile_id,
            VERSION.protocol_version,
        )
        .await
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
    protocol_version: i32,
) -> anyhow::Result<(RawConnection, ServerProfile)> {
    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(protocol_version),
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

    let Some(switch_mode) = SwitchMode::of(protocol_version) else {
        bail!("no login switch mode is known for protocol version {protocol_version}");
    };
    // A server that never announced a threshold still can ask for compression.
    let mut stored_threshold = CompressionThreshold(0);
    let (profile, options) = loop {
        let (id, body) = io.recv_frame().await?;
        let mut r = &body[..];
        if id == ClientboundLoginDisconnect::ID {
            let packet = ClientboundLoginDisconnect::decode(&mut r)?;
            bail!("server refused the login: {}", packet.reason.0);
        } else if id == LoginCompression::ID {
            let threshold = CompressionThreshold(LoginCompression::decode(&mut r)?.threshold.0);
            match switch_mode {
                SwitchMode::Immediate => io.set_compression(threshold),
                SwitchMode::Acknowledged => stored_threshold = threshold,
            }
        } else if id == ClientboundLoginFinished::ID {
            let (profile, options) = match switch_mode {
                SwitchMode::Immediate => (
                    ClientboundLoginFinished::decode(&mut r)?.profile,
                    OptionFlags::new(),
                ),
                SwitchMode::Acknowledged => {
                    let packet = ClientboundLoginFinishedWithOptions::decode(&mut r)?;
                    (packet.profile, packet.options)
                }
            };
            break (
                ServerProfile {
                    id: profile.id,
                    username: profile.username.0.to_owned(),
                },
                options,
            );
        } else {
            bail!("unexpected login packet {id:#04x}; only offline login is supported");
        }
    };

    if options.cipher() {
        bail!("the server asked for encryption; only offline login is supported");
    }
    let compression = options.compression().then_some(stored_threshold);
    if let Some(threshold) = compression {
        io.set_inbound_compression(threshold);
    }
    io.send_packet(&ServerboundLoginAcknowledged).await?;
    if let Some(threshold) = compression {
        io.set_outbound_compression(threshold);
    }
    info!("logged in to {host}:{port} as {}", profile.username);
    Ok((io.into_raw_connection(remote_addr, switch_mode), profile))
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
                ConfigurationPackets::default(),
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
                Ok(Some(packet)) => {
                    let event = ReceivedPacketEvent {
                        entity,
                        id: packet.id,
                        data: packet.payload,
                        timestamp: packet.timestamp,
                    };
                    commands.trigger(event);
                }
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
        &ConnectionState,
        &mut ConfigurationPackets,
    )>,
    holds_vanilla: Option<Res<HoldsVanillaPack>>,
    mut commands: Commands,
) {
    let Ok((mut connection, state, mut collected)) = connections.get_mut(event.entity) else {
        return;
    };
    if *state != ConnectionState::Configuration {
        return;
    }

    if let Some(offer) = event.decode::<ClientboundSelectKnownPacks>() {
        let vanilla = KnownPack {
            namespace: "minecraft",
            id: "core",
            version: VERSION.id.as_str(),
        };
        let known_packs = if holds_vanilla.is_some() && offer.known_packs.contains(&vanilla) {
            vec![vanilla]
        } else {
            Vec::new()
        };
        collected.vanilla_known = !known_packs.is_empty();
        connection.write_packet(&ServerboundSelectKnownPacks { known_packs });
    } else if let Some(data) = event.decode::<ClientboundRegistryData>() {
        collect_registry(&mut collected.received, data);
    } else if let Some(update) = event.decode::<ClientboundUpdateTags>() {
        collected
            .received
            .replace_tags(received_tags(update.registries));
    } else if let Some(keep_alive) = event.decode::<ConfigurationKeepAlive>() {
        connection.write_packet(&ServerboundConfigurationKeepAlive(KeepAlive {
            payload: keep_alive.0.payload,
        }));
    } else if event.decode::<ClientboundFinishConfiguration>().is_some() {
        let ConfigurationPackets {
            received,
            vanilla_known,
        } = std::mem::take(&mut *collected);
        commands.trigger(RegistriesReceived {
            entity: event.entity,
            registries: received,
            vanilla_known,
        });
    }
}

fn refuse_packet(commands: &mut Commands, connection: Entity, name: &str, error: anyhow::Error) {
    close_connection_later(
        commands,
        connection,
        format!("the server sent a {name} the client cannot read: {error:#}"),
    );
}

fn in_session(
    registries: Option<&mcrs_minecraft_registries::Registries>,
    spawn: &PlayerSpawnInfo,
) -> anyhow::Result<()> {
    let id = spawn.dimension_type_id;
    anyhow::ensure!(
        registries.is_some_and(|registries| Namer::name(registries, id).is_some()),
        "dimension type {} is not an entry of the session registries",
        id.number()
    );
    Ok(())
}

fn handle_game_packet(
    event: On<ReceivedPacketEvent>,
    mut connections: Query<(
        &mut ClientConnection,
        &mut ConnectionState,
        &mut PendingTeleports,
        &mut ConfigurationPackets,
    )>,
    registries: Option<Res<CurrentRegistries>>,
    mut commands: Commands,
) {
    let session = registries.as_deref().map(|registries| &**registries);
    let Ok((mut connection, mut state, mut pending_teleports, mut collected)) =
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
    } else if let Some(login) = event.try_decode::<ClientboundLogin>() {
        match login.and_then(|login| in_session(session, &login.player_spawn_info).map(|()| login))
        {
            Ok(login) => {
                commands.entity(event.entity).insert((
                    JoinedGame {
                        player_id: login.player_id,
                        dimensions: login.dimensions,
                    },
                    CurrentDimension::of(&login.player_spawn_info),
                ));
            }
            Err(error) => refuse_packet(&mut commands, event.entity, ClientboundLogin::NAME, error),
        }
    } else if let Some(respawn) = event.try_decode::<ClientboundRespawn>() {
        let respawn = respawn
            .and_then(|respawn| in_session(session, &respawn.player_spawn_info).map(|()| respawn));
        match respawn {
            Ok(respawn) => {
                commands
                    .entity(event.entity)
                    .insert(CurrentDimension::of(&respawn.player_spawn_info));
            }
            Err(error) => {
                refuse_packet(&mut commands, event.entity, ClientboundRespawn::NAME, error)
            }
        }
    } else if let Some(Err(error)) = event.try_decode::<ClientboundSetEntityData>().map(|data| {
        let data = data?;
        anyhow::ensure!(
            session.is_some_and(|registries| data.metadata.names_entries_of(registries)),
            "an id is not an entry of the session registries"
        );
        Ok(data)
    }) {
        refuse_packet(
            &mut commands,
            event.entity,
            ClientboundSetEntityData::NAME,
            error,
        );
    } else if event.decode::<ClientboundStartConfiguration>().is_some() {
        connection.write_packet(&ServerboundConfigurationAcknowledged);
        *state = ConnectionState::Configuration;
        *collected = ConfigurationPackets::default();
    } else if let Some(update) = event.decode::<ClientboundGameUpdateTags>() {
        commands.trigger(TagsReceived {
            entity: event.entity,
            tags: received_tags(update.registries),
        });
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
    use bytes::BytesMut;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_dimension::keys::DIMENSION_TYPE;
    use mcrs_minecraft_dimension::{Dimension, DimensionType};
    use mcrs_minecraft_entity::keys::CAT_VARIANT;
    use mcrs_minecraft_entity::variant::CatVariant;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_protocol::GameMode;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol::decode::PacketDecoder;
    use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
    use mcrs_minecraft_protocol::packets::configuration::clientbound::{RegistryTags, TagGroup};
    use mcrs_minecraft_protocol::packets::game::clientbound::{
        ClientboundStartConfiguration, ClientboundUpdateTags as GameUpdateTags,
    };
    use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
    use mcrs_minecraft_protocol::registry::Entry;
    use mcrs_minecraft_protocol::text::Text;
    use mcrs_minecraft_registries::{Declaration, Registries};
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn runtime() -> Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

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
                ConfigurationPackets::default(),
                PendingTeleports::default(),
            ))
            .id();
        (app, inbound, entity)
    }

    #[test]
    fn a_closed_connection_ends_the_session() {
        let runtime = runtime();
        let (mut app, inbound, _) = client_app(&runtime);
        app.update();
        assert!(app.should_exit().is_none());
        drop(inbound);
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
    }

    #[test]
    fn a_disconnect_packet_ends_the_session() {
        let runtime = runtime();
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

        let runtime = runtime();
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

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Probe {
        size: i32,
    }

    const PROBE_NAME: &str = "minecraft:test_probe";

    fn name(text: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::read(text).unwrap()
    }

    fn session(cats: usize) -> CurrentRegistries {
        let types = [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
            "minecraft:beta",
        ]
        .map(name)
        .to_vec();
        let cats = (0..cats)
            .map(|n| name(&format!("minecraft:cat{n}")))
            .collect();
        let declarations = vec![
            Declaration::fixed(DIMENSION_TYPE.location().into(), types),
            Declaration::fixed(CAT_VARIANT.location().into(), cats),
        ];
        let (registries, _) =
            Registries::receive(declarations, &Received::default(), None).unwrap();
        CurrentRegistries::new(registries)
    }

    fn statics() -> CurrentRegistries {
        session(0)
    }

    fn dimension_type(number: u16) -> Id<DimensionType> {
        Id::checked(number, 8).unwrap()
    }

    fn probe(size: i32) -> NbtTag {
        mcrs_minecraft_nbt::to_nbt_tag(&Probe { size }).unwrap()
    }

    fn probe_data(entries: &[(&str, Option<i32>)]) -> ClientboundRegistryData<'static> {
        ClientboundRegistryData {
            registry: ResourceLocation::read_cow(PROBE_NAME.to_owned()).unwrap(),
            entries: entries
                .iter()
                .map(|(entry, size)| Entry {
                    id: ResourceLocation::read_cow((*entry).to_owned()).unwrap(),
                    data: size.map(|size| Cow::Owned(probe(size))),
                })
                .collect(),
        }
    }

    fn tag_groups(registry: &str, tags: &[(&str, &[u16])]) -> Vec<RegistryTags<'static>> {
        vec![RegistryTags {
            registry: ResourceLocation::read_cow(registry.to_owned()).unwrap(),
            tags: tags
                .iter()
                .map(|(tag, members)| TagGroup {
                    name: ResourceLocation::read_cow((*tag).to_owned()).unwrap(),
                    entries: members.iter().map(|&m| VarInt(i32::from(m))).collect(),
                })
                .collect(),
        }]
    }

    fn received(registry: &str, tags: &[(&str, &[u16])]) -> ReceivedTags {
        vec![(
            name(registry),
            tags.iter()
                .map(|(tag, members)| (name(tag), members.iter().map(|&m| i32::from(m)).collect()))
                .collect(),
        )]
    }

    #[derive(Resource, Default)]
    struct Handed {
        registries: Vec<(Received, bool)>,
        tags: Vec<ReceivedTags>,
    }

    /// Stands in for the client crate: takes what the server sent, makes the statics the decode
    /// scope and finishes.
    fn accept_registries(
        event: On<RegistriesReceived>,
        mut handed: bevy_ecs::prelude::ResMut<Handed>,
        mut commands: Commands,
    ) {
        handed
            .registries
            .push((event.registries.clone(), event.vanilla_known));
        commands.insert_resource(statics());
        finish_configuration(&mut commands, event.entity);
    }

    fn accept_tags(event: On<TagsReceived>, mut handed: bevy_ecs::prelude::ResMut<Handed>) {
        handed.tags.push(event.tags.clone());
    }

    struct Harness {
        app: App,
        inbound: mpsc::Sender<crate::ReceivedPacket>,
        outgoing: mpsc::Receiver<bytes::Bytes>,
        entity: Entity,
    }

    impl Harness {
        fn new(runtime: &Runtime, state: ConnectionState) -> Self {
            let _guard = runtime.enter();
            let (raw, outgoing, inbound) = RawConnection::new_for_test_full(8);
            let mut app = App::new();
            app.insert_resource(ExitOnDisconnect);
            app.init_resource::<Handed>();
            app.add_systems(Update, (receive_packets, flush).chain());
            app.add_observer(handle_configuration_packet);
            app.add_observer(handle_game_packet);
            app.add_observer(accept_registries);
            app.add_observer(accept_tags);
            let entity = app
                .world_mut()
                .spawn((
                    ClientConnection { raw: Box::new(raw) },
                    state,
                    ConfigurationPackets::default(),
                    PendingTeleports::default(),
                ))
                .id();
            Harness {
                app,
                inbound,
                outgoing,
                entity,
            }
        }

        fn configuring(runtime: &Runtime) -> Self {
            Harness::new(runtime, ConnectionState::Configuration)
        }

        fn playing(runtime: &Runtime) -> Self {
            Harness::new(runtime, ConnectionState::Game)
        }

        fn deliver<P: Encode + Packet>(&self, runtime: &Runtime, packet: &P) {
            deliver(runtime, &self.inbound, packet);
        }

        fn sent(&mut self) -> Vec<mcrs_minecraft_protocol::decode::PacketFrame> {
            let mut decoder = PacketDecoder::new();
            while let Ok(blob) = self.outgoing.try_recv() {
                decoder.queue_bytes(BytesMut::from(&blob[..]));
            }
            std::iter::from_fn(|| decoder.try_next_packet().unwrap()).collect()
        }

        fn sent_ids(&mut self) -> Vec<i32> {
            self.sent().iter().map(|frame| frame.id).collect()
        }

        fn state(&self) -> ConnectionState {
            *self
                .app
                .world()
                .get::<ConnectionState>(self.entity)
                .unwrap()
        }

        fn dropped(&self) -> bool {
            self.app.world().get_entity(self.entity).is_err()
        }

        fn handed(&self) -> &Handed {
            self.app.world().resource::<Handed>()
        }
    }

    fn login(player_id: i32, dimension_type: Id<DimensionType>) -> ClientboundLogin {
        ClientboundLogin {
            player_id,
            hardcore: false,
            dimensions: vec![mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into()],
            max_players: VarInt(1),
            chunk_radius: VarInt(8),
            simulation_distance: VarInt(8),
            reduced_debug_info: false,
            show_death_screen: true,
            do_limited_crafting: false,
            player_spawn_info: PlayerSpawnInfo::new(
                dimension_type,
                mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
                GameMode::Survival,
            ),
            online_mode: false,
            enforces_secure_chat: false,
        }
    }

    fn respawn(
        dimension: ResourceKey<Dimension>,
        dimension_type: Id<DimensionType>,
    ) -> ClientboundRespawn {
        ClientboundRespawn {
            player_spawn_info: PlayerSpawnInfo::new(dimension_type, dimension, GameMode::Survival),
            data_to_keep: 0,
        }
    }

    #[test]
    fn the_end_of_configuration_hands_over_what_the_server_sent_in_order() {
        let runtime = runtime();
        let mut client = Harness::configuring(&runtime);
        client.deliver(
            &runtime,
            &probe_data(&[("minecraft:zeta", Some(0)), ("minecraft:alpha", None)]),
        );
        client.deliver(&runtime, &probe_data(&[("minecraft:beta", Some(2))]));
        client.deliver(
            &runtime,
            &ClientboundUpdateTags {
                registries: tag_groups(PROBE_NAME, &[("minecraft:old", &[0])]),
            },
        );
        client.deliver(
            &runtime,
            &ClientboundUpdateTags {
                registries: tag_groups(PROBE_NAME, &[("minecraft:t", &[2, 0])]),
            },
        );
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();

        assert_eq!(
            client.handed().registries,
            [(
                Received {
                    registries: vec![(
                        name(PROBE_NAME),
                        vec![
                            (name("minecraft:zeta"), Some(probe(0))),
                            (name("minecraft:alpha"), None),
                            (name("minecraft:beta"), Some(probe(2))),
                        ]
                    )],
                    tags: received(PROBE_NAME, &[("minecraft:t", &[2, 0])]),
                },
                false
            )]
        );
        assert_eq!(client.state(), ConnectionState::Game);
        assert_eq!(client.sent_ids(), [ServerboundFinishConfiguration::ID]);
    }

    #[test]
    fn the_vanilla_pack_is_accepted_only_by_a_client_that_holds_it() {
        use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundSelectKnownPacks;

        let runtime = runtime();
        let vanilla = || KnownPack {
            namespace: "minecraft",
            id: "core",
            version: VERSION.id.as_str(),
        };
        let other = KnownPack {
            namespace: "minecraft",
            id: "core",
            version: "an-older-snapshot",
        };
        let answer = |holds: bool, offered: Vec<KnownPack<'static>>| {
            let mut client = Harness::configuring(&runtime);
            if holds {
                client.app.insert_resource(HoldsVanillaPack);
            }
            client.deliver(
                &runtime,
                &ClientboundSelectKnownPacks {
                    known_packs: offered,
                },
            );
            client.deliver(&runtime, &ClientboundFinishConfiguration);
            client.app.update();
            let sent = client.sent();
            assert_eq!(sent[0].id, ServerboundSelectKnownPacks::ID);
            let answered = ServerboundSelectKnownPacks::decode(&mut &sent[0].body[..])
                .unwrap()
                .known_packs
                .len();
            (answered, client.handed().registries[0].1)
        };

        assert_eq!(answer(false, vec![vanilla()]), (0, false));
        assert_eq!(answer(true, vec![vanilla()]), (1, true));
        assert_eq!(answer(true, vec![other]), (0, false));
    }

    #[test]
    fn a_login_in_the_frame_of_the_finish_is_read_against_the_session_registries() {
        let runtime = runtime();
        let mut client = Harness::configuring(&runtime);
        client.deliver(&runtime, &probe_data(&[("minecraft:a", Some(0))]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.deliver(&runtime, &login(7, dimension_type(2)));
        client.app.update();

        let current = client
            .app
            .world()
            .get::<CurrentDimension>(client.entity)
            .expect("the login named a dimension type of the session set");
        assert_eq!(current.dimension_type.index(), 2);
    }

    #[test]
    fn a_respawn_moves_the_current_dimension_to_the_key_and_type_it_names() {
        use mcrs_minecraft_dimension::keys::dimension::{OVERWORLD, THE_NETHER};

        let runtime = runtime();
        let mut client = Harness::playing(&runtime);
        client.app.insert_resource(statics());
        let current = |client: &Harness| {
            client
                .app
                .world()
                .get::<CurrentDimension>(client.entity)
                .cloned()
        };

        client.deliver(&runtime, &login(7, dimension_type(3)));
        client.app.update();
        let first = current(&client).expect("the login names type 3");
        assert_eq!(first.key, OVERWORLD);
        assert_eq!(first.dimension_type.index(), 3);

        client.deliver(&runtime, &respawn(THE_NETHER.into(), dimension_type(1)));
        client.app.update();
        let second = current(&client).expect("the respawn names type 1");
        assert_eq!(second.key, THE_NETHER);
        assert_eq!(second.dimension_type.index(), 1);
        assert_eq!(
            client
                .app
                .world()
                .get::<JoinedGame>(client.entity)
                .unwrap()
                .player_id,
            7
        );

        client.deliver(&runtime, &respawn(OVERWORLD.into(), dimension_type(4)));
        client.app.update();
        assert!(client.dropped(), "a type number past the dimension types");
    }

    #[test]
    fn a_cat_variant_outside_the_session_registry_drops_the_connection() {
        use mcrs_minecraft_protocol::entity::{MetaDataValue, Metadata, MetadataEntry};

        let entity_data = |variant: u16| ClientboundSetEntityData {
            entity_id: VarInt(5),
            metadata: Metadata(vec![MetadataEntry {
                index: 20,
                value: MetaDataValue::CatVariant(Id::<CatVariant>::checked(variant, 3).unwrap()),
            }]),
        };

        let runtime = runtime();
        let mut client = Harness::playing(&runtime);
        client.app.insert_resource(session(2));

        client.deliver(&runtime, &entity_data(1));
        client.app.update();
        assert!(!client.dropped(), "the last cat variant of the session");

        client.deliver(&runtime, &entity_data(2));
        client.app.update();
        assert!(client.dropped(), "one past the session's cat variants");
    }

    #[test]
    fn a_login_naming_a_type_outside_the_session_registries_drops_the_connection() {
        let runtime = runtime();
        let mut client = Harness::playing(&runtime);
        client.app.insert_resource(statics());
        client.deliver(&runtime, &login(7, dimension_type(4)));
        client.app.update();
        assert!(client.dropped());

        let mut without = Harness::playing(&runtime);
        without.deliver(&runtime, &login(7, dimension_type(0)));
        without.app.update();
        assert!(without.dropped(), "no session registries at all");
    }

    #[test]
    fn re_entering_configuration_acknowledges_and_collects_afresh() {
        let runtime = runtime();
        let mut client = Harness::configuring(&runtime);
        client.deliver(&runtime, &probe_data(&[("minecraft:a", Some(0))]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();
        client.sent_ids();

        client.deliver(&runtime, &ClientboundStartConfiguration);
        client.app.update();
        assert_eq!(client.state(), ConnectionState::Configuration);
        assert_eq!(
            client.sent_ids(),
            [ServerboundConfigurationAcknowledged::ID]
        );

        client.deliver(&runtime, &probe_data(&[("minecraft:c", Some(1))]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();
        assert_eq!(
            client.handed().registries[1].0.registries,
            [(
                name(PROBE_NAME),
                vec![(name("minecraft:c"), Some(probe(1)))]
            )]
        );
        assert_eq!(client.state(), ConnectionState::Game);
    }

    #[test]
    fn tag_updates_in_play_are_handed_over_in_the_order_sent() {
        let runtime = runtime();
        let mut client = Harness::playing(&runtime);
        let play_tags = |registry, tags| GameUpdateTags {
            registries: tag_groups(registry, tags),
        };
        client.deliver(&runtime, &play_tags(PROBE_NAME, &[("minecraft:t", &[1])]));
        client.deliver(
            &runtime,
            &play_tags("minecraft:dimension_type", &[("minecraft:d", &[2])]),
        );
        client.app.update();

        assert_eq!(
            client.handed().tags,
            [
                received(PROBE_NAME, &[("minecraft:t", &[1])]),
                received("minecraft:dimension_type", &[("minecraft:d", &[2])]),
            ]
        );
        assert!(!client.dropped());
    }

    #[test]
    fn without_the_resource_a_closed_connection_only_logs() {
        let runtime = runtime();
        let (mut app, inbound, _) = client_app(&runtime);
        app.world_mut().remove_resource::<ExitOnDisconnect>();
        drop(inbound);
        app.update();
        assert!(app.should_exit().is_none());
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod log_in_tests {
    use super::*;
    use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
    use mcrs_minecraft_protocol::profile::GameProfile;
    use std::time::Duration;

    #[test]
    fn an_acknowledged_switch_login_compresses_each_direction_at_its_own_boundary() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let mut io = PacketIo::new(listener.accept().await.unwrap().0);
                let handshake = io.recv_packet::<ServerboundHandshake>().await.unwrap();
                assert_eq!(
                    handshake.protocol_version.0,
                    crate::PROTOCOL_26_4_SNAPSHOT_5
                );
                io.recv_frame().await.unwrap();
                io.send_packet(&LoginCompression {
                    threshold: VarInt(256),
                })
                .await
                .unwrap();
                io.send_packet(&ClientboundLoginFinishedWithOptions {
                    profile: GameProfile {
                        id: Uuid::nil(),
                        username: Bounded("Steve"),
                        properties: Default::default(),
                    },
                    session_id: Uuid::nil(),
                    options: OptionFlags::new().with_compression(true),
                })
                .await
                .unwrap();
                let (id, _) = io.recv_frame().await.unwrap();
                assert_eq!(
                    id,
                    ServerboundLoginAcknowledged::ID,
                    "an uncompressed acknowledgement"
                );
                io.set_compression(CompressionThreshold(256));
                io
            });

            let io = PacketIo::connect(address).await.unwrap();
            let (mut connection, profile) = log_in(
                io,
                address,
                "127.0.0.1".to_owned(),
                address.port(),
                "Steve".to_owned(),
                Uuid::nil(),
                crate::PROTOCOL_26_4_SNAPSHOT_5,
            )
            .await
            .unwrap();
            assert_eq!(profile.username, "Steve");
            let mut server = server.await.unwrap();

            let json = "a".repeat(1000);
            let large = StatusResponse { json: &json };
            connection.append(&large).unwrap();
            connection.flush().unwrap();
            let (id, body) = server.recv_frame().await.unwrap();
            assert_eq!((id, body.len()), (StatusResponse::ID, 1002));

            server.send_packet(&large).await.unwrap();
            let received = tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    if let Some(packet) = connection.try_recv().unwrap() {
                        return packet;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                (received.id, received.payload.len()),
                (StatusResponse::ID, 1002)
            );
        });
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
}
