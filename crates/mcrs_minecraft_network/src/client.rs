use crate::event::{Received, Trigger};
use crate::join::{Configuration, Join, Joined, received_tags};
use crate::metrics::PacketDispatcher;
use crate::packet_io::PacketIo;
use crate::{ConnectionState, RawConnection};
use anyhow::Context as _;
use bevy_app::{App, AppExit, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::prelude::{Commands, On, Query, Res};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_core::{ResourceKey, VERSION};
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::common::serverbound::KeepAlive;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundBundleDelimiter, ClientboundChunkCacheRadius, ClientboundDisconnect,
    ClientboundKeepAlive as GameKeepAlive, ClientboundLevelChunkWithLight, ClientboundLogin,
    ClientboundPlayerPosition, ClientboundRespawn, ClientboundSetChunkCacheCenter,
    ClientboundSetEntityData, ClientboundStartConfiguration,
    ClientboundUpdateTags as ClientboundGameUpdateTags,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundConfigurationAcknowledged, ServerboundKeepAlive as ServerboundGameKeepAlive,
};
use mcrs_minecraft_protocol::packets::table::Row;
use mcrs_minecraft_protocol::{Encode, Packet, WritePacket, uuid::Uuid};
use mcrs_minecraft_registries::{CurrentRegistries, Namer, ReceivedTags};
use md5::{Digest, Md5};
use std::net::SocketAddr;
#[cfg(not(target_family = "wasm"))]
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{Receiver, channel};
use tracing::{error, warn};

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
    /// Inserts [`HoldsVanillaPack`]. Set it only when the registry loader can fill the entries
    /// a server leaves out.
    pub holds_vanilla_pack: bool,
}

/// The socket, its reader and writer tasks, and the outbound encoder.
#[derive(Component)]
pub struct ClientConnection {
    raw: Box<RawConnection>,
    dispatcher: PacketDispatcher,
    /// The frames of the bundle that the server opened and has not closed yet. A frame of a row
    /// without a structure counts towards the limit and raises nothing.
    bundle: Option<Vec<(Option<Trigger>, Bytes, crate::Instant)>>,
    /// The state of the last frame that ended one. A frame behind it is read only once the
    /// state changes, because an observer can answer the end later than the same frame.
    ended: Option<ConnectionState>,
}

impl ClientConnection {
    fn new(raw: RawConnection, ended: Option<ConnectionState>) -> Self {
        Self {
            raw: Box::new(raw),
            // A column is large; its observer decodes it off the main thread.
            dispatcher: PacketDispatcher::clientbound()
                .defer::<ClientboundLevelChunkWithLight<'static>>(),
            bundle: None,
            ended,
        }
    }
}

pub use crate::join::ServerProfile;

/// Present when the client holds the vanilla pack of its own version, so it may let a server
/// leave that pack's entries out.
#[derive(Resource)]
pub struct HoldsVanillaPack;

/// A server ended its configuration. The connection stays in configuration until an observer
/// answers with [`finish_configuration`] or drops it with [`close_connection_later`].
#[derive(EntityEvent, Debug)]
pub struct RegistriesReceived {
    pub entity: Entity,
    pub configuration: Configuration,
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
struct ClientRuntime(Runtime);

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
        #[cfg(not(target_family = "wasm"))]
        match Runtime::new() {
            Ok(runtime) => {
                app.insert_resource(ClientRuntime(runtime));
            }
            Err(error) => {
                tracing::error!("the client network runtime did not start: {error}");
                app.world_mut().write_message(AppExit::error());
                return;
            }
        }
        app.configure_sets(
            Update,
            (ClientNetworkSystems::Receive, ClientNetworkSystems::Flush).chain(),
        );
        app.add_systems(Update, flush.in_set(ClientNetworkSystems::Flush));
        add_packet_observers(app);
        if self.holds_vanilla_pack {
            app.insert_resource(HoldsVanillaPack);
        }

        let (send, recv) = channel(1);
        #[cfg_attr(
            not(target_family = "wasm"),
            expect(
                clippy::clone_on_copy,
                reason = "ServerAddress is Copy only on native; the browser's WebTransportTarget is not"
            )
        )]
        let server = self.server.clone();
        #[cfg(not(target_family = "wasm"))]
        let (server_address, server_port, remote_addr) =
            (server.ip().to_string(), server.port(), server);
        // A browser never learns the peer address; the connection carries one
        // only so a server-side log line has something to print.
        #[cfg(target_family = "wasm")]
        let ((server_address, server_port), remote_addr) =
            (server.host_and_port(), SocketAddr::from(([0, 0, 0, 0], 0)));
        let join = Join {
            server_address,
            server_port,
            remote_addr,
            username: self.username.clone(),
            profile_id: self
                .profile_id
                .unwrap_or_else(|| offline_player_uuid(&self.username)),
            protocol_version: VERSION.protocol_version,
            view_distance: self.view_distance,
            holds_vanilla_pack: self.holds_vanilla_pack,
        };
        let joining = async move {
            let outcome = connect_and_join(server, join)
                .await
                .map_err(|e| format!("{e:#}"));
            let _ = send.send(outcome).await;
        };
        #[cfg(not(target_family = "wasm"))]
        app.world().resource::<ClientRuntime>().0.spawn(joining);
        #[cfg(target_family = "wasm")]
        wasm_bindgen_futures::spawn_local(joining);

        app.add_systems(
            Update,
            (spawn_joined_connection(recv), receive_packets)
                .chain()
                .in_set(ClientNetworkSystems::Receive),
        );
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

async fn connect_and_join(server: ServerAddress, join: Join) -> anyhow::Result<Joined> {
    #[cfg(not(target_family = "wasm"))]
    let io = PacketIo::connect(server).await?;
    #[cfg(target_family = "wasm")]
    let io = PacketIo::new(crate::browser::connect(&server).await?);
    crate::join::configure(io, join).await
}

type JoinOutcome = Result<Joined, String>;

fn spawn_joined_connection(mut joined: Receiver<JoinOutcome>) -> impl FnMut(&mut World) {
    move |world: &mut World| {
        while let Ok(outcome) = joined.try_recv() {
            let Joined {
                connection,
                profile,
                configuration,
            } = match outcome {
                Ok(joined) => joined,
                Err(e) => {
                    end_session(world, format!("the connection failed: {e}"));
                    continue;
                }
            };
            let entity = world
                .spawn((
                    // The server ended the configuration while the client joined.
                    ClientConnection::new(connection, Some(ConnectionState::Configuration)),
                    ConnectionState::Configuration,
                    profile,
                    Configuration::default(),
                    PendingTeleports::default(),
                ))
                .id();
            world.trigger(RegistriesReceived {
                entity,
                configuration,
            });
        }
    }
}

/// The vanilla client refuses a bundle longer than this.
const MAX_BUNDLE_PACKETS: usize = 4096;

fn receive_packets(
    mut connections: Query<(Entity, &mut ClientConnection, &ConnectionState)>,
    mut commands: Commands,
) {
    for (entity, mut connection, &state) in connections.iter_mut() {
        let connection = &mut *connection;
        match connection.ended {
            // The held frames stay unread, but a closed socket still ends the session.
            Some(ended) if ended == state => {
                if connection.raw.reader_closed() {
                    close_connection_later(
                        &mut commands,
                        entity,
                        "the server closed the connection".into(),
                    );
                }
                continue;
            }
            Some(_) => connection.ended = None,
            None => {}
        }
        loop {
            let packet = match connection.raw.try_recv() {
                Ok(Some(packet)) => packet,
                Ok(None) => break,
                Err(_) => {
                    close_connection_later(
                        &mut commands,
                        entity,
                        "the server closed the connection".into(),
                    );
                    break;
                }
            };
            let trigger = match connection
                .dispatcher
                .dispatch(state, packet.id, &packet.payload)
            {
                Ok(trigger) => trigger,
                Err(malformed) => {
                    if let Some(line) = &malformed.log {
                        warn!("{line}");
                    }
                    close_connection_later(
                        &mut commands,
                        entity,
                        format!("Internal Exception: {}", malformed.cause),
                    );
                    break;
                }
            };
            if state == ConnectionState::Game && packet.id == ClientboundBundleDelimiter::ID {
                match connection.bundle.take() {
                    None => connection.bundle = Some(Vec::new()),
                    Some(bundle) => raise_bundle(&mut commands, entity, bundle),
                }
                continue;
            }
            let ends_state = connection.dispatcher.ends_state(state, packet.id);
            match &mut connection.bundle {
                Some(_) if ends_state => {
                    close_connection_later(
                        &mut commands,
                        entity,
                        "Terminal message received in bundle".into(),
                    );
                    break;
                }
                Some(bundle) if bundle.len() >= MAX_BUNDLE_PACKETS => {
                    close_connection_later(
                        &mut commands,
                        entity,
                        "Too many packets in a bundle".into(),
                    );
                    break;
                }
                Some(bundle) => bundle.push((trigger, packet.payload, packet.timestamp)),
                None => {
                    if let Some(trigger) = trigger {
                        trigger.raise(&mut commands, entity, packet.payload, packet.timestamp);
                    }
                }
            }
            if ends_state {
                connection.ended = Some(state);
                break;
            }
        }
    }
}

fn raise_bundle(
    commands: &mut Commands,
    entity: Entity,
    bundle: Vec<(Option<Trigger>, Bytes, crate::Instant)>,
) {
    for (trigger, data, timestamp) in bundle {
        if let Some(trigger) = trigger {
            trigger.raise(commands, entity, data, timestamp);
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

pub(crate) fn add_packet_observers(app: &mut App) {
    use mcrs_minecraft_protocol::packets::configuration::clientbound as configuration;
    app.add_observer(
        on_configuration_packet::<configuration::ClientboundSelectKnownPacks<'static>>,
    );
    app.add_observer(on_configuration_packet::<configuration::ClientboundRegistryData<'static>>);
    app.add_observer(on_configuration_packet::<configuration::ClientboundUpdateTags<'static>>);
    app.add_observer(on_configuration_packet::<configuration::ClientboundKeepAlive>);
    app.add_observer(on_configuration_packet::<configuration::ClientboundDisconnect>);
    app.add_observer(on_configuration_packet::<configuration::ClientboundFinishConfiguration>);
    app.add_observer(on_disconnect);
    app.add_observer(on_login);
    app.add_observer(on_respawn);
    app.add_observer(on_set_entity_data);
    app.add_observer(on_start_configuration);
    app.add_observer(on_update_tags);
    app.add_observer(on_player_position);
    app.add_observer(on_set_chunk_cache_center);
    app.add_observer(on_chunk_cache_radius);
    app.add_observer(on_keep_alive);
}

/// The asynchronous join reads the same rows by id, so the rows go to its reader.
fn on_configuration_packet<R: Row>(
    event: On<Received<R>>,
    mut connections: Query<(&mut ClientConnection, &mut Configuration)>,
    holds_vanilla: Option<Res<HoldsVanillaPack>>,
    mut commands: Commands,
) {
    let Ok((mut connection, mut collected)) = connections.get_mut(event.entity) else {
        return;
    };
    match collected.read(
        <R::Packet<'static> as Packet>::ID,
        event.data(),
        holds_vanilla.is_some(),
        &mut *connection,
    ) {
        Ok(None) => {}
        Ok(Some(configuration)) => commands.trigger(RegistriesReceived {
            entity: event.entity,
            configuration,
        }),
        Err(error) => close_connection_later(&mut commands, event.entity, format!("{error:#}")),
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

fn on_disconnect(event: On<Received<ClientboundDisconnect>>, mut commands: Commands) {
    let Some(disconnect) = event.packet() else {
        return;
    };
    close_connection_later(
        &mut commands,
        event.entity,
        format!(
            "the server disconnected you: {}",
            disconnect.reason.to_legacy_lossy()
        ),
    );
}

fn on_login(
    event: On<Received<ClientboundLogin>>,
    registries: Option<Res<CurrentRegistries>>,
    mut commands: Commands,
) {
    let Some(login) = event.packet() else {
        return;
    };
    let session = registries.as_deref().map(|registries| &**registries);
    match in_session(session, &login.player_spawn_info) {
        Ok(()) => {
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
}

fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    registries: Option<Res<CurrentRegistries>>,
    mut commands: Commands,
) {
    let Some(respawn) = event.packet() else {
        return;
    };
    let session = registries.as_deref().map(|registries| &**registries);
    match in_session(session, &respawn.player_spawn_info) {
        Ok(()) => {
            commands
                .entity(event.entity)
                .insert(CurrentDimension::of(&respawn.player_spawn_info));
        }
        Err(error) => refuse_packet(&mut commands, event.entity, ClientboundRespawn::NAME, error),
    }
}

fn on_set_entity_data(
    event: On<Received<ClientboundSetEntityData<'static>>>,
    registries: Option<Res<CurrentRegistries>>,
    mut commands: Commands,
) {
    let Some(data) = event.packet() else {
        return;
    };
    let checked = match registries.as_deref() {
        Some(registries) => data
            .metadata
            .check(registries)
            .context("an id is not an entry of the session registries"),
        None => Err(anyhow::anyhow!(
            "an id is not an entry of the session registries"
        )),
    };
    if let Err(error) = checked {
        refuse_packet(
            &mut commands,
            event.entity,
            ClientboundSetEntityData::NAME,
            error,
        );
    }
}

fn on_start_configuration(
    event: On<Received<ClientboundStartConfiguration>>,
    mut connections: Query<(
        &mut ClientConnection,
        &mut ConnectionState,
        &mut Configuration,
    )>,
) {
    let Ok((mut connection, mut state, mut collected)) = connections.get_mut(event.entity) else {
        return;
    };
    connection.write_packet(&ServerboundConfigurationAcknowledged);
    *state = ConnectionState::Configuration;
    *collected = Configuration::default();
}

fn on_update_tags(event: On<Received<ClientboundGameUpdateTags<'static>>>, mut commands: Commands) {
    let Some(update) = event.packet() else {
        return;
    };
    commands.trigger(TagsReceived {
        entity: event.entity,
        tags: received_tags(update.registries),
    });
}

fn on_player_position(
    event: On<Received<ClientboundPlayerPosition>>,
    mut pending_teleports: Query<&mut PendingTeleports>,
) {
    let (Some(position), Ok(mut pending_teleports)) =
        (event.packet(), pending_teleports.get_mut(event.entity))
    else {
        return;
    };
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
}

fn on_set_chunk_cache_center(
    event: On<Received<ClientboundSetChunkCacheCenter>>,
    mut commands: Commands,
) {
    let Some(center) = event.packet() else {
        return;
    };
    commands
        .entity(event.entity)
        .insert(ChunkCacheCenter(ColumnPos::new(center.x.0, center.z.0)));
}

fn on_chunk_cache_radius(event: On<Received<ClientboundChunkCacheRadius>>, mut commands: Commands) {
    let Some(radius) = event.packet() else {
        return;
    };
    commands
        .entity(event.entity)
        .insert(ChunkCacheRadius(radius.radius.0));
}

fn on_keep_alive(
    event: On<Received<GameKeepAlive>>,
    mut connections: Query<&mut ClientConnection>,
) {
    let (Some(keep_alive), Ok(mut connection)) =
        (event.packet(), connections.get_mut(event.entity))
    else {
        return;
    };
    connection.write_packet(&ServerboundGameKeepAlive(KeepAlive {
        payload: keep_alive.0.payload,
    }));
}

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod tests {
    use super::*;
    use bevy_app::Update;
    use bytes::BytesMut;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_dimension::keys::DIMENSION_TYPE;
    use mcrs_minecraft_dimension::{Dimension, DimensionType};
    use mcrs_minecraft_entity::keys::CAT_VARIANT;
    use mcrs_minecraft_entity::variant::CatVariant;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_protocol::Decode;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol::decode::PacketDecoder;
    use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
    use mcrs_minecraft_protocol::packets::configuration::clientbound::{
        ClientboundFinishConfiguration, ClientboundRegistryData, ClientboundSelectKnownPacks,
        ClientboundUpdateTags, RegistryTags, TagGroup,
    };
    use mcrs_minecraft_protocol::packets::game::clientbound::{
        ClientboundStartConfiguration, ClientboundUpdateTags as GameUpdateTags,
    };
    use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
    use mcrs_minecraft_protocol::registry::Entry;
    use mcrs_minecraft_protocol::resource_pack::KnownPack;
    use mcrs_minecraft_protocol::text::Text;
    use mcrs_minecraft_registries::{Declaration, Received, Registries};
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
        add_packet_observers(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                ClientConnection::new(raw, None),
                ConnectionState::Game,
                Configuration::default(),
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
        handed.registries.push((
            event.configuration.registries.clone(),
            event.configuration.vanilla_known,
        ));
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
            add_packet_observers(&mut app);
            app.add_observer(accept_registries);
            app.add_observer(accept_tags);
            let entity = app
                .world_mut()
                .spawn((
                    ClientConnection::new(raw, None),
                    state,
                    Configuration::default(),
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
            version: VERSION.id,
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
    fn a_login_behind_the_finish_is_read_against_the_session_registries() {
        let runtime = runtime();
        let mut client = Harness::configuring(&runtime);
        client.deliver(&runtime, &probe_data(&[("minecraft:a", Some(0))]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.deliver(&runtime, &login(7, dimension_type(2)));
        client.app.update();
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
    fn a_state_change_inside_a_bundle_drops_the_connection() {
        let runtime = runtime();
        let mut client = Harness::playing(&runtime);
        client.deliver(&runtime, &ClientboundBundleDelimiter);
        client.deliver(&runtime, &ClientboundStartConfiguration);
        client.deliver(&runtime, &ClientboundBundleDelimiter);
        client.app.update();

        assert!(client.dropped());
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
