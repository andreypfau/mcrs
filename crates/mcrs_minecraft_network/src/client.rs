use crate::event::ReceivedPacketEvent;
use crate::identity;
use crate::packet_io::{ByteStream, PacketIo};
use crate::{ConnectionState, RawConnection};
use anyhow::bail;
use bevy_app::{App, AppExit, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, On, Query, Res};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::world::World;
use bevy_math::DVec3;
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
    ClientboundLoginDisconnect, ClientboundLoginFinished, LoginCompression,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::{
    Bounded, CompressionThreshold, Decode, Encode, Look, Packet, VarInt, WritePacket, uuid::Uuid,
};
use mcrs_minecraft_registry::{
    Id, KnownPackEntries, LoadReport, NetworkEntry, NetworkRegistry, NetworkTags, RegistrySet,
    WorldRegistries,
};
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

/// What the client reads a server's registries with: the declarations of the
/// registries it syncs, the registries it holds without the server, and the
/// pack whose entries a server may leave out.
#[derive(Resource)]
pub struct SessionRegistryInputs {
    pub declarations: WorldRegistries,
    pub statics: RegistrySet,
    pub known: Option<KnownPackEntries>,
}

/// The registry and tag packets of the configuration in progress, kept until
/// its end builds the session set from them.
#[derive(Component, Default)]
struct ConfigurationPackets {
    registries: Vec<NetworkRegistry>,
    tags: Vec<NetworkTags>,
    vanilla_accepted: bool,
}

impl ConfigurationPackets {
    /// Entries a server leaves out are read from the vanilla pack only when this client told it
    /// to leave them out; otherwise they have no data to be read from.
    fn known<'a>(&self, inputs: &'a SessionRegistryInputs) -> Option<&'a KnownPackEntries> {
        inputs.known.as_ref().filter(|_| self.vanilla_accepted)
    }

    fn collect_registry(&mut self, registry: NetworkRegistry) {
        match self
            .registries
            .iter_mut()
            .find(|collected| collected.registry == registry.registry)
        {
            Some(collected) => collected.entries.extend(registry.entries),
            None => self.registries.push(registry),
        }
    }

    fn collect_tags(&mut self, update: Vec<RegistryTags<'_>>) {
        for registry in update.into_iter().map(network_tags) {
            match self
                .tags
                .iter_mut()
                .find(|collected| collected.registry == registry.registry)
            {
                Some(collected) => *collected = registry,
                None => self.tags.push(registry),
            }
        }
    }
}

fn network_registry(data: ClientboundRegistryData<'_>) -> NetworkRegistry {
    NetworkRegistry {
        registry: data.registry.into(),
        entries: data
            .entries
            .into_iter()
            .map(|entry| NetworkEntry {
                name: entry.id.into(),
                data: entry.data.map(Cow::into_owned),
            })
            .collect(),
    }
}

fn network_tags(registry: RegistryTags<'_>) -> NetworkTags {
    NetworkTags {
        registry: registry.registry.into(),
        tags: registry
            .tags
            .into_iter()
            .map(|group| {
                let members = group
                    .entries
                    .into_iter()
                    .map(|member| i32::from(member.0))
                    .collect();
                (group.name.into(), members)
            })
            .collect(),
    }
}

fn build_session_registries(
    inputs: &SessionRegistryInputs,
    collected: ConfigurationPackets,
) -> Result<RegistrySet, LoadReport> {
    let known = collected.known(inputs);
    inputs.declarations.from_network(
        &inputs.statics,
        collected.registries,
        &collected.tags,
        known,
    )
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
                    commands.queue(move |world: &mut World| trigger_in_session_scope(world, event));
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

/// Ids in a play packet are numbers of the session registries, so every observer of the packet
/// decodes inside the set the previous packet left behind.
fn trigger_in_session_scope(world: &mut World, event: ReceivedPacketEvent) {
    match world.get_resource::<RegistrySet>().cloned() {
        Some(session) => session.scope(|| world.trigger(event)),
        None => world.trigger(event),
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
        &mut ConfigurationPackets,
    )>,
    inputs: Option<Res<SessionRegistryInputs>>,
    mut commands: Commands,
) {
    let Ok((mut connection, mut state, mut collected)) = connections.get_mut(event.entity) else {
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
        let holds_vanilla = inputs
            .as_deref()
            .is_some_and(|inputs| inputs.known.is_some());
        let known_packs = if holds_vanilla && offer.known_packs.contains(&vanilla) {
            vec![vanilla]
        } else {
            Vec::new()
        };
        collected.vanilla_accepted = !known_packs.is_empty();
        connection.write_packet(&ServerboundSelectKnownPacks { known_packs });
    } else if let Some(data) = event.decode::<ClientboundRegistryData>() {
        collected.collect_registry(network_registry(data));
    } else if let Some(update) = event.decode::<ClientboundUpdateTags>() {
        collected.collect_tags(update.registries);
    } else if let Some(keep_alive) = event.decode::<ConfigurationKeepAlive>() {
        connection.write_packet(&ServerboundConfigurationKeepAlive(KeepAlive {
            payload: keep_alive.0.payload,
        }));
    } else if event.decode::<ClientboundFinishConfiguration>().is_some() {
        let collected = std::mem::take(&mut *collected);
        let built = match inputs.as_deref() {
            Some(inputs) => build_session_registries(inputs, collected)
                .map_err(|report| format!("the server's registries cannot be used:\n{report}")),
            None => Err("the client has nothing to read the server's registries with".to_owned()),
        };
        match built {
            Ok(registries) => {
                commands.insert_resource(registries);
                connection.write_packet(&ServerboundFinishConfiguration);
                *state = ConnectionState::Game;
            }
            Err(reason) => close_connection_later(&mut commands, event.entity, reason),
        }
    }
}

fn refuse_packet(commands: &mut Commands, connection: Entity, name: &str, error: anyhow::Error) {
    close_connection_later(
        commands,
        connection,
        format!("the server sent a {name} the client cannot read: {error:#}"),
    );
}

fn handle_game_packet(
    event: On<ReceivedPacketEvent>,
    mut connections: Query<(
        &mut ClientConnection,
        &mut ConnectionState,
        &mut PendingTeleports,
        &mut ConfigurationPackets,
    )>,
    registries: Option<Res<RegistrySet>>,
    mut commands: Commands,
) {
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
        match login {
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
    } else if let Some(Err(error)) = event.try_decode::<ClientboundSetEntityData>() {
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
        let tags: Vec<_> = update.registries.into_iter().map(network_tags).collect();
        match registries
            .as_deref()
            .map(|set| set.with_network_tags(&tags))
        {
            Some(Ok(replaced)) => commands.insert_resource(replaced),
            Some(Err(report)) => close_connection_later(
                &mut commands,
                event.entity,
                format!("the server's tags cannot be used:\n{report}"),
            ),
            None => close_connection_later(
                &mut commands,
                event.entity,
                "the server sent tags before its registries".to_owned(),
            ),
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
    use bytes::BytesMut;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::rl;
    use mcrs_minecraft_core::tag_key::TagKey;
    use mcrs_minecraft_dimension::keys::DIMENSION_TYPE;
    use mcrs_minecraft_dimension::{Dimension, DimensionType};
    use mcrs_minecraft_protocol::GameMode;
    use mcrs_minecraft_protocol::RegistryId;
    use mcrs_minecraft_protocol::decode::PacketDecoder;
    use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
    use mcrs_minecraft_protocol::packets::configuration::clientbound::{RegistryTags, TagGroup};
    use mcrs_minecraft_protocol::packets::game::clientbound::{
        ClientboundStartConfiguration, ClientboundUpdateTags as GameUpdateTags,
    };
    use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
    use mcrs_minecraft_protocol::registry::Entry;
    use mcrs_minecraft_protocol::text::Text;
    use mcrs_minecraft_registry::Registry;
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;
    use std::sync::Arc;
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
                ConfigurationPackets::default(),
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

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Probe {
        size: i32,
    }

    const PROBE: RegistryKey<Probe> = RegistryKey::new(rl!("minecraft:test_probe"));
    const PROBE_NAME: &str = "minecraft:test_probe";

    fn name(text: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::read(text).unwrap()
    }

    fn dimension_types() -> Registry<DimensionType> {
        Registry::new(
            DIMENSION_TYPE,
            [
                "minecraft:overworld",
                "minecraft:the_nether",
                "minecraft:the_end",
                "minecraft:beta",
            ]
            .map(name),
        )
        .unwrap()
    }

    fn statics() -> RegistrySet {
        RegistrySet::new()
            .with(dimension_types())
            .unwrap()
            .with_types([PROBE.binding()])
            .unwrap()
    }

    fn declarations() -> WorldRegistries {
        let mut registries = WorldRegistries::new([name(PROBE_NAME)]);
        registries
            .parse::<Probe>(PROBE.location())
            .sync_value::<Probe, _>(PROBE.location(), Clone::clone)
            .receive::<Probe, _>(PROBE.location(), |probe| (probe,));
        registries
    }

    fn inputs() -> SessionRegistryInputs {
        SessionRegistryInputs {
            declarations: declarations(),
            statics: statics(),
            known: None,
        }
    }

    fn probe_data(entries: &[&str]) -> ClientboundRegistryData<'static> {
        ClientboundRegistryData {
            registry: ResourceLocation::read_cow(PROBE_NAME.to_owned()).unwrap(),
            entries: entries
                .iter()
                .enumerate()
                .map(|(size, entry)| Entry {
                    id: ResourceLocation::read_cow((*entry).to_owned()).unwrap(),
                    data: Some(Cow::Owned(
                        mcrs_minecraft_nbt::to_nbt_tag(&Probe { size: size as i32 }).unwrap(),
                    )),
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
                    entries: members.iter().copied().map(RegistryId).collect(),
                })
                .collect(),
        }]
    }

    fn configuration_tags(
        registry: &str,
        tags: &[(&str, &[u16])],
    ) -> ClientboundUpdateTags<'static> {
        ClientboundUpdateTags {
            registries: tag_groups(registry, tags),
        }
    }

    fn play_tags(registry: &str, tags: &[(&str, &[u16])]) -> GameUpdateTags<'static> {
        GameUpdateTags {
            registries: tag_groups(registry, tags),
        }
    }

    fn members<R: 'static>(set: &RegistrySet, tag: &str) -> Vec<usize> {
        let tags = set.tags::<R>().unwrap();
        let tag = tags
            .get(&TagKey::<R, _>::from_location(name(tag)))
            .expect("the tag exists");
        tags.members(tag).map(|id| id.index()).collect()
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
            app.add_systems(Update, (receive_packets, flush).chain());
            app.add_observer(handle_configuration_packet);
            app.add_observer(handle_game_packet);
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
            let mut harness = Harness::new(runtime, ConnectionState::Configuration);
            harness.app.insert_resource(inputs());
            harness
        }

        fn playing(runtime: &Runtime) -> Self {
            Harness::new(runtime, ConnectionState::Game)
        }

        fn deliver<P: Encode + Packet>(&self, runtime: &Runtime, packet: &P) {
            deliver(runtime, &self.inbound, packet);
        }

        fn sent_ids(&mut self) -> Vec<i32> {
            let mut decoder = PacketDecoder::new();
            while let Ok(blob) = self.outgoing.try_recv() {
                decoder.queue_bytes(BytesMut::from(&blob[..]));
            }
            std::iter::from_fn(|| decoder.try_next_packet().unwrap())
                .map(|frame| frame.id)
                .collect()
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

        fn session(&self) -> RegistrySet {
            self.app.world().resource::<RegistrySet>().clone()
        }
    }

    fn login(player_id: i32, dimension_type: u16) -> ClientboundLogin {
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
                Id::from_raw(dimension_type),
                mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
                GameMode::Survival,
            ),
            online_mode: false,
            enforces_secure_chat: false,
        }
    }

    fn respawn(dimension: ResourceKey<Dimension>, dimension_type: u16) -> ClientboundRespawn {
        ClientboundRespawn {
            player_spawn_info: PlayerSpawnInfo::new(
                Id::from_raw(dimension_type),
                dimension,
                GameMode::Survival,
            ),
            data_to_keep: 0,
        }
    }

    #[test]
    fn the_end_of_configuration_inserts_the_session_registries_before_acknowledging() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::configuring(&runtime);
        client.deliver(
            &runtime,
            &probe_data(&["minecraft:zeta", "minecraft:alpha"]),
        );
        client.deliver(
            &runtime,
            &configuration_tags(PROBE_NAME, &[("minecraft:t", &[1, 0])]),
        );
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();

        let set = client.session();
        let table = set.table(PROBE_NAME).unwrap();
        assert_eq!(table.number("minecraft:zeta"), Some(0));
        assert_eq!(table.number("minecraft:alpha"), Some(1));
        assert_eq!(members::<Probe>(&set, "minecraft:t"), [1, 0]);
        assert!(set.registry::<DimensionType>().is_some(), "the statics");
        assert_eq!(client.state(), ConnectionState::Game);
        assert_eq!(
            client.sent_ids(),
            [ServerboundFinishConfiguration::ID],
            "the acknowledgement is the only thing written"
        );
    }

    #[test]
    fn a_registry_split_across_packets_is_read_as_one() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::configuring(&runtime);
        client.deliver(&runtime, &probe_data(&["minecraft:zeta"]));
        client.deliver(&runtime, &probe_data(&["minecraft:alpha"]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();

        assert_eq!(client.state(), ConnectionState::Game);
        let set = client.session();
        let table = set.table(PROBE_NAME).unwrap();
        assert_eq!(table.number("minecraft:zeta"), Some(0));
        assert_eq!(table.number("minecraft:alpha"), Some(1));

        let mut collected = ConfigurationPackets::default();
        for entries in [["minecraft:one"], ["minecraft:one"]] {
            collected.collect_registry(network_registry(probe_data(&entries)));
        }
        let report = build_session_registries(&inputs(), collected)
            .err()
            .expect("the repeated entry is refused")
            .to_string();
        assert!(
            report.contains("minecraft:test_probe/minecraft:one"),
            "{report}"
        );
    }

    #[test]
    fn a_registry_entry_that_does_not_parse_drops_the_connection() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::configuring(&runtime);
        let mut data = probe_data(&["minecraft:good", "minecraft:bad"]);
        data.entries[1].data = Some(Cow::Owned(mcrs_minecraft_nbt::to_nbt_tag(&"text").unwrap()));
        client.deliver(&runtime, &data);
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();

        assert!(client.dropped());
        assert!(!client.app.world().contains_resource::<RegistrySet>());
        assert_eq!(client.app.should_exit(), Some(AppExit::Success));
        assert_eq!(client.sent_ids(), Vec::<i32>::new());

        let report = build_session_registries(
            &inputs(),
            ConfigurationPackets {
                registries: vec![network_registry(data)],
                ..Default::default()
            },
        )
        .err()
        .expect("the entry is refused")
        .to_string();
        assert!(
            report.contains("minecraft:test_probe/minecraft:bad"),
            "{report}"
        );
    }

    #[test]
    fn the_vanilla_pack_is_accepted_only_by_a_client_that_holds_its_entries() {
        use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundSelectKnownPacks;

        let runtime = Runtime::new().unwrap();
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
        let answer = |known: Option<KnownPackEntries>, offered: Vec<KnownPack<'static>>| {
            let mut client = Harness::configuring(&runtime);
            client
                .app
                .insert_resource(SessionRegistryInputs { known, ..inputs() });
            client.deliver(
                &runtime,
                &ClientboundSelectKnownPacks {
                    known_packs: offered,
                },
            );
            client.app.update();
            let mut decoder = PacketDecoder::new();
            while let Ok(blob) = client.outgoing.try_recv() {
                decoder.queue_bytes(BytesMut::from(&blob[..]));
            }
            let frame = decoder.try_next_packet().unwrap().expect("an answer");
            assert_eq!(frame.id, ServerboundSelectKnownPacks::ID);
            let answered = ServerboundSelectKnownPacks::decode(&mut &frame.body[..])
                .unwrap()
                .known_packs
                .len();
            let world = client.app.world();
            let reads_known = world
                .get::<ConfigurationPackets>(client.entity)
                .unwrap()
                .known(world.resource::<SessionRegistryInputs>())
                .is_some();
            (answered, reads_known)
        };

        assert_eq!(answer(None, vec![vanilla()]), (0, false));
        assert_eq!(
            answer(Some(KnownPackEntries::default()), vec![vanilla()]),
            (1, true)
        );
        assert_eq!(
            answer(Some(KnownPackEntries::default()), vec![other]),
            (0, false),
            "entries a server sends without data are not read from a pack it was not told we hold"
        );
    }

    #[test]
    fn without_declarations_the_end_of_configuration_drops_the_connection() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::new(&runtime, ConnectionState::Configuration);
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();

        assert!(client.dropped());
        assert!(!client.app.world().contains_resource::<RegistrySet>());
        assert_eq!(client.sent_ids(), Vec::<i32>::new());
    }

    #[test]
    fn a_login_in_the_frame_of_the_finish_is_read_against_the_session_registries() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::configuring(&runtime);
        client.deliver(&runtime, &probe_data(&["minecraft:a"]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.deliver(&runtime, &login(7, 2));
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

        let runtime = Runtime::new().unwrap();
        let mut client = Harness::playing(&runtime);
        client.app.insert_resource(statics());
        let current = |client: &Harness| {
            client
                .app
                .world()
                .get::<CurrentDimension>(client.entity)
                .cloned()
        };

        client.deliver(&runtime, &login(7, 3));
        client.app.update();
        let first = current(&client).expect("the login names type 3");
        assert_eq!(first.key, OVERWORLD);
        assert_eq!(first.dimension_type.index(), 3);

        client.deliver(&runtime, &respawn(THE_NETHER.into(), 1));
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

        client.deliver(&runtime, &respawn(OVERWORLD.into(), 4));
        client.app.update();
        assert!(client.dropped(), "a type number past the dimension types");
    }

    #[test]
    fn a_cat_variant_outside_the_session_registry_drops_the_connection() {
        use mcrs_minecraft_entity::keys::CAT_VARIANT;
        use mcrs_minecraft_entity::variant::CatVariant;
        use mcrs_minecraft_protocol::entity::{MetaDataValue, Metadata, MetadataEntry};

        let cats = Registry::<CatVariant>::new(
            CAT_VARIANT,
            ["minecraft:tabby", "minecraft:red"].map(name),
        )
        .unwrap();
        let entity_data = |variant: u16| ClientboundSetEntityData {
            entity_id: VarInt(5),
            metadata: Metadata(vec![MetadataEntry {
                index: 20,
                value: MetaDataValue::CatVariant(Id::from_raw(variant)),
            }]),
        };

        let runtime = Runtime::new().unwrap();
        let mut client = Harness::playing(&runtime);
        client.app.insert_resource(statics().with(cats).unwrap());

        client.deliver(&runtime, &entity_data(1));
        client.app.update();
        assert!(!client.dropped(), "the last cat variant of the session");

        client.deliver(&runtime, &entity_data(2));
        client.app.update();
        assert!(client.dropped(), "one past the session's cat variants");
    }

    #[test]
    fn a_login_naming_a_type_outside_the_session_registries_drops_the_connection() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::playing(&runtime);
        client.app.insert_resource(statics());
        client.deliver(&runtime, &login(7, 4));
        client.app.update();
        assert!(client.dropped());

        let mut without = Harness::playing(&runtime);
        without.deliver(&runtime, &login(7, 0));
        without.app.update();
        assert!(without.dropped(), "no session registries at all");
    }

    fn session_of_probe(entries: &[&str], tags: &[(&str, &[u16])]) -> RegistrySet {
        let tags = vec![network_tags(tag_groups(PROBE_NAME, tags).pop().unwrap())];
        build_session_registries(
            &inputs(),
            ConfigurationPackets {
                registries: vec![network_registry(probe_data(entries))],
                tags,
                ..Default::default()
            },
        )
        .unwrap_or_else(|report| panic!("{report}"))
    }

    #[test]
    fn re_entering_configuration_acknowledges_and_rebuilds_at_its_end() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::configuring(&runtime);
        client.deliver(&runtime, &probe_data(&["minecraft:a", "minecraft:b"]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();
        assert_eq!(client.state(), ConnectionState::Game);
        assert_eq!(client.session().table(PROBE_NAME).unwrap().len(), 2);
        client.sent_ids();

        client.deliver(&runtime, &ClientboundStartConfiguration);
        client.app.update();
        assert_eq!(client.state(), ConnectionState::Configuration);
        assert_eq!(
            client.sent_ids(),
            [ServerboundConfigurationAcknowledged::ID]
        );
        assert_eq!(
            client.session().table(PROBE_NAME).unwrap().len(),
            2,
            "the previous set stays until the new one replaces it"
        );

        client.deliver(&runtime, &probe_data(&["minecraft:c"]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();
        let set = client.session();
        let table = set.table(PROBE_NAME).unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table.number("minecraft:c"), Some(0));
        assert_eq!(table.number("minecraft:a"), None);
        assert_eq!(client.state(), ConnectionState::Game);
        assert_eq!(client.sent_ids(), [ServerboundFinishConfiguration::ID]);
    }

    #[test]
    fn a_tag_update_in_play_replaces_the_session_registries() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::playing(&runtime);
        let before = session_of_probe(
            &["minecraft:a", "minecraft:b", "minecraft:c"],
            &[("minecraft:t", &[0])],
        )
        .with_network_tags(&[NetworkTags {
            registry: name("minecraft:dimension_type"),
            tags: vec![(name("minecraft:d"), vec![1])],
        }])
        .unwrap();
        client.app.insert_resource(before.clone());

        client.deliver(
            &runtime,
            &play_tags(PROBE_NAME, &[("minecraft:t", &[2, 1])]),
        );
        client.app.update();

        let after = client.session();
        assert_eq!(members::<Probe>(&after, "minecraft:t"), [2, 1]);
        assert_eq!(members::<DimensionType>(&after, "minecraft:d"), [1]);
        assert!(!Arc::ptr_eq(
            after.tag_table(PROBE_NAME).unwrap(),
            before.tag_table(PROBE_NAME).unwrap()
        ));
        assert_eq!(
            members::<Probe>(&before, "minecraft:t"),
            [0],
            "a holder of the previous set still sees it whole"
        );
        assert!(!client.dropped());
    }

    #[test]
    fn tag_updates_arriving_in_one_frame_all_apply() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::playing(&runtime);
        client.app.insert_resource(session_of_probe(
            &["minecraft:a", "minecraft:b"],
            &[("minecraft:t", &[0])],
        ));

        client.deliver(&runtime, &play_tags(PROBE_NAME, &[("minecraft:t", &[1])]));
        client.deliver(
            &runtime,
            &play_tags("minecraft:dimension_type", &[("minecraft:d", &[2])]),
        );
        client.app.update();

        let after = client.session();
        assert_eq!(members::<Probe>(&after, "minecraft:t"), [1]);
        assert_eq!(members::<DimensionType>(&after, "minecraft:d"), [2]);
    }

    #[test]
    fn a_tag_update_naming_an_id_outside_its_registry_drops_the_connection() {
        let runtime = Runtime::new().unwrap();
        let mut client = Harness::playing(&runtime);
        client
            .app
            .insert_resource(session_of_probe(&["minecraft:a"], &[("minecraft:t", &[0])]));

        client.deliver(
            &runtime,
            &play_tags(PROBE_NAME, &[("minecraft:t", &[0, 7])]),
        );
        client.app.update();

        assert!(client.dropped());
        assert_eq!(
            members::<Probe>(&client.session(), "minecraft:t"),
            [0],
            "the refused update changes nothing"
        );
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
}
