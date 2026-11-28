use crate::event::{Received, Trigger};
use crate::join::{Configuration, Join, Joined};
use crate::metrics::{PacketDispatcher, PlayTable};
use crate::packet_io::PacketIo;
use crate::{ConnectionState, RawConnection};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::message::{Message, MessageReader};
use bevy_ecs::prelude::{Commands, On, Query, Res, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::Populated;
use bevy_ecs::world::World;
use bytes::Bytes;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_lang::Language;
use mcrs_minecraft_protocol::packets::common::serverbound::KeepAlive;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundBundleDelimiter, ClientboundDisconnect, ClientboundKeepAlive as GameKeepAlive,
    ClientboundStartConfiguration,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundConfigurationAcknowledged, ServerboundKeepAlive as ServerboundGameKeepAlive,
};
use mcrs_minecraft_protocol::packets::table::Row;
use mcrs_minecraft_protocol::{Encode, Packet, WritePacket, uuid::Uuid};
use mcrs_minecraft_text::Text;
use md5::{Digest, Md5};
use std::fmt;
use std::future::{Future, poll_fn};
use std::net::SocketAddr;
use std::pin::pin;
use std::task::Poll;
#[cfg(not(target_family = "wasm"))]
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

#[cfg(not(target_family = "wasm"))]
pub type ServerAddress = SocketAddr;
#[cfg(target_family = "wasm")]
pub type ServerAddress = crate::browser::WebTransportTarget;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientNetworkSystems {
    Receive,
    Deliver,
    Flush,
}

pub struct ClientNetworkPlugin {
    pub holds_vanilla_pack: bool,
    pub play: Option<PlayTable>,
}

#[derive(Component)]
pub struct ClientConnection {
    raw: Box<RawConnection>,
    dispatcher: PacketDispatcher,
    bundle: Option<Vec<(Option<Trigger>, Bytes)>>,
    ended: Option<ConnectionState>,
}

impl ClientConnection {
    fn new(
        raw: RawConnection,
        ended: Option<ConnectionState>,
        dispatcher: PacketDispatcher,
    ) -> Self {
        Self {
            raw: Box::new(raw),
            dispatcher,
            bundle: None,
            ended,
        }
    }
}

fn client_dispatcher(
    play: Option<&ClientPlayTable>,
    deferred: Option<&DeferredRows>,
) -> PacketDispatcher {
    let mut dispatcher = PacketDispatcher::clientbound();
    if let Some(play) = play {
        dispatcher.set_play(play.0);
    }
    deferred
        .into_iter()
        .flat_map(|deferred| &deferred.0)
        .fold(dispatcher, |dispatcher, defer| defer(dispatcher))
}

#[derive(Resource)]
struct ClientPlayTable(PlayTable);

#[derive(Resource, Default)]
pub struct DeferredRows(Vec<fn(PacketDispatcher) -> PacketDispatcher>);

impl DeferredRows {
    pub fn defer<R: Row>(&mut self) {
        self.0.push(PacketDispatcher::defer::<R>);
    }
}

pub use crate::join::ServerProfile;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentViewDistance(pub u8);

#[derive(Resource)]
pub struct HoldsVanillaPack;

#[derive(EntityEvent, Debug)]
pub struct RegistriesReceived {
    pub entity: Entity,
    pub configuration: Configuration,
}

pub fn finish_configuration(mut commands: Commands, connection: Entity) {
    commands.queue(move |world: &mut World| {
        let Ok(mut entity) = world.get_entity_mut(connection) else {
            return;
        };
        if let Some(mut client) = entity.get_mut::<ClientConnection>() {
            client.write_packet(&ServerboundFinishConfiguration);
        }
        if let Some(mut state) = entity.get_mut::<ConnectionState>() {
            state.set_if_neq(ConnectionState::Game);
        }
    });
}

#[cfg(not(target_family = "wasm"))]
#[derive(Resource)]
struct ClientRuntime(Runtime);

#[derive(Message, Clone, Debug)]
pub struct JoinRequest {
    pub server: ServerAddress,
    pub username: String,
    pub profile_id: Option<Uuid>,
    pub view_distance: u8,
}

#[derive(Message, Clone, Copy, Debug, Default)]
pub struct LeaveRequest;

#[derive(Message, Clone, Copy, Debug, Default)]
pub struct CancelJoin;

#[derive(Message, Clone, Debug)]
pub struct ConnectionEnded {
    pub reason: ConnectionEnd,
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum ConnectionEnd {
    Left,
    Kicked(Text),
    JoinFailed(String),
    Lost(String),
}

impl fmt::Display for ConnectionEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionEnd::Left => f.write_str("the client left the server"),
            ConnectionEnd::Kicked(reason) => write!(
                f,
                "the server disconnected you: {}",
                mcrs_minecraft_text_flatten::plain_text(&Language::default(), reason)
            ),
            ConnectionEnd::JoinFailed(cause) => write!(f, "the connection failed: {cause}"),
            ConnectionEnd::Lost(cause) => f.write_str(cause),
        }
    }
}

#[derive(Component, Debug)]
pub struct ClientSession;

type JoinOutcome = Result<Joined, String>;

#[derive(Component)]
struct PendingJoin {
    joined: oneshot::Receiver<JoinOutcome>,
    view_distance: u8,
}

pub fn close_connection_later(mut commands: Commands, connection: Entity, reason: ConnectionEnd) {
    commands.queue(move |world: &mut World| {
        if world.despawn(connection) {
            match reason {
                ConnectionEnd::Left => info!("{reason}"),
                _ => error!("{reason}"),
            }
            world.write_message(ConnectionEnded { reason });
        }
    });
}

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<JoinRequest>()
            .add_message::<LeaveRequest>()
            .add_message::<CancelJoin>()
            .add_message::<ConnectionEnded>();
        #[cfg(not(target_family = "wasm"))]
        match Runtime::new() {
            Ok(runtime) => {
                app.insert_resource(ClientRuntime(runtime));
            }
            Err(error) => {
                tracing::error!("the client network runtime did not start: {error}");
                app.world_mut().write_message(bevy_app::AppExit::error());
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
        if let Some(play) = self.play {
            app.insert_resource(ClientPlayTable(play));
        }

        app.add_systems(
            Update,
            (
                leave_sessions.run_if(on_message::<LeaveRequest>),
                cancel_joins.run_if(on_message::<CancelJoin>),
                start_joins.run_if(on_message::<JoinRequest>),
                (collect_joins, receive_packets)
                    .chain()
                    .in_set(ClientNetworkSystems::Deliver),
            )
                .chain()
                .in_set(ClientNetworkSystems::Receive),
        );
    }
}

fn leave_sessions(sessions: Query<Entity, With<ClientSession>>, mut commands: Commands) {
    for session in &sessions {
        close_connection_later(commands.reborrow(), session, ConnectionEnd::Left);
    }
}

fn cancel_joins(pending: Query<Entity, With<PendingJoin>>, mut commands: Commands) {
    for join in &pending {
        commands.entity(join).despawn();
    }
}

fn start_joins(
    mut requests: MessageReader<JoinRequest>,
    holds_vanilla: Option<Res<HoldsVanillaPack>>,
    #[cfg(not(target_family = "wasm"))] runtime: Res<ClientRuntime>,
    pending: Query<Entity, With<PendingJoin>>,
    mut commands: Commands,
) {
    if let Some(request) = requests.read().last() {
        for earlier in &pending {
            commands.entity(earlier).despawn();
        }
        #[cfg_attr(
            not(target_family = "wasm"),
            expect(
                clippy::clone_on_copy,
                reason = "ServerAddress is Copy only on native; the browser's WebTransportTarget is not"
            )
        )]
        let server = request.server.clone();
        #[cfg(not(target_family = "wasm"))]
        let (server_address, server_port, remote_addr) =
            (server.ip().to_string(), server.port(), server);
        #[cfg(target_family = "wasm")]
        let ((server_address, server_port), remote_addr) =
            (server.host_and_port(), SocketAddr::from(([0, 0, 0, 0], 0)));
        let join = Join {
            server_address,
            server_port,
            remote_addr,
            username: request.username.clone(),
            profile_id: request
                .profile_id
                .unwrap_or_else(|| offline_player_uuid(&request.username)),
            protocol_version: VERSION.protocol_version,
            view_distance: request.view_distance,
            holds_vanilla_pack: holds_vanilla.is_some(),
        };
        let (send, joined) = oneshot::channel();
        let joining = async move {
            let mut send = send;
            let mut joining = pin!(connect_and_join(server, join));
            let outcome = poll_fn(|cx| {
                if send.poll_closed(cx).is_ready() {
                    return Poll::Ready(None);
                }
                joining.as_mut().poll(cx).map(Some)
            })
            .await;
            if let Some(outcome) = outcome {
                let _ = send.send(outcome.map_err(|e| format!("{e:#}")));
            }
        };
        #[cfg(not(target_family = "wasm"))]
        runtime.0.spawn(joining);
        #[cfg(target_family = "wasm")]
        wasm_bindgen_futures::spawn_local(joining);
        commands.spawn((
            ClientSession,
            PendingJoin {
                joined,
                view_distance: request.view_distance,
            },
        ));
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the allocations happen only when a join completes or fails"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(5))]
fn collect_joins(
    mut pending: Populated<(Entity, &mut PendingJoin)>,
    play: Option<Res<ClientPlayTable>>,
    deferred: Option<Res<DeferredRows>>,
    mut commands: Commands,
) {
    for (session, mut join) in &mut pending {
        let outcome = match join.joined.try_recv() {
            Ok(outcome) => outcome,
            Err(oneshot::error::TryRecvError::Empty) => continue,
            Err(oneshot::error::TryRecvError::Closed) => {
                Err("the join stopped without an outcome".to_owned())
            }
        };
        let Joined {
            connection,
            profile,
            configuration,
        } = match outcome {
            Ok(joined) => joined,
            Err(e) => {
                close_connection_later(commands.reborrow(), session, ConnectionEnd::JoinFailed(e));
                continue;
            }
        };
        commands.entity(session).remove::<PendingJoin>().insert((
            ClientConnection::new(
                connection,
                Some(ConnectionState::Configuration),
                client_dispatcher(play.as_deref(), deferred.as_deref()),
            ),
            ConnectionState::Configuration,
            profile,
            Configuration::default(),
            SentViewDistance(join.view_distance),
        ));
        commands.trigger(RegistriesReceived {
            entity: session,
            configuration,
        });
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

const MAX_BUNDLE_PACKETS: usize = 4096;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the allocations happen only on a bundle start, on a malformed packet or when the connection ends"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(19))]
fn receive_packets(
    mut connections: Populated<(Entity, &mut ClientConnection, &ConnectionState)>,
    mut commands: Commands,
) {
    for (entity, mut connection, &state) in connections.iter_mut() {
        let connection = &mut *connection;
        match connection.ended {
            Some(ended) if ended == state => {
                if connection.raw.reader_closed() {
                    close_connection_later(
                        commands.reborrow(),
                        entity,
                        ConnectionEnd::Lost("the server closed the connection".into()),
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
                        commands.reborrow(),
                        entity,
                        ConnectionEnd::Lost("the server closed the connection".into()),
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
                        commands.reborrow(),
                        entity,
                        ConnectionEnd::Lost(format!("Internal Exception: {}", malformed.cause)),
                    );
                    break;
                }
            };
            if state == ConnectionState::Game && packet.id == ClientboundBundleDelimiter::ID {
                match connection.bundle.take() {
                    None => connection.bundle = Some(Vec::new()),
                    Some(bundle) => raise_bundle(commands.reborrow(), entity, bundle),
                }
                continue;
            }
            let ends_state = connection.dispatcher.ends_state(state, packet.id);
            match &mut connection.bundle {
                Some(_) if ends_state => {
                    close_connection_later(
                        commands.reborrow(),
                        entity,
                        ConnectionEnd::Lost("Terminal message received in bundle".into()),
                    );
                    break;
                }
                Some(bundle) if bundle.len() >= MAX_BUNDLE_PACKETS => {
                    close_connection_later(
                        commands.reborrow(),
                        entity,
                        ConnectionEnd::Lost("Too many packets in a bundle".into()),
                    );
                    break;
                }
                Some(bundle) => bundle.push((trigger, packet.payload)),
                None => {
                    if let Some(trigger) = trigger {
                        trigger.raise(commands.reborrow(), entity, packet.payload);
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
    mut commands: Commands<'_, '_>,
    entity: Entity,
    bundle: Vec<(Option<Trigger>, Bytes)>,
) {
    for (trigger, data) in bundle {
        if let Some(trigger) = trigger {
            trigger.raise(commands.reborrow(), entity, data);
        }
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the reason string exists only when the connection to the server fails"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the hand-off stamp measures wall-clock time for the stalled-writer check, which must not depend on the game clock"
    )
)]
fn flush(mut connections: Populated<(Entity, &mut ClientConnection)>, mut commands: Commands) {
    for (entity, mut connection) in connections.iter_mut() {
        if let Err(e) = connection.raw.flush() {
            warn!("failed to send to the server: {e}");
        }
        if connection.raw.disconnected() {
            close_connection_later(
                commands.reborrow(),
                entity,
                ConnectionEnd::Lost("the connection to the server failed".into()),
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
    app.add_observer(on_start_configuration);
    app.add_observer(on_keep_alive);
}

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
        Err(error) => close_connection_later(
            commands.reborrow(),
            event.entity,
            ConnectionEnd::Lost(format!("{error:#}")),
        ),
    }
}

fn on_disconnect(event: On<Received<ClientboundDisconnect>>, mut commands: Commands) {
    let Some(disconnect) = event.packet() else {
        return;
    };
    close_connection_later(
        commands.reborrow(),
        event.entity,
        ConnectionEnd::Kicked(disconnect.reason),
    );
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
    state.set_if_neq(ConnectionState::Configuration);
    collected.set_if_neq(Configuration::default());
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
    use crate::OutboundReceiver;
    use bevy_app::Update;
    use bevy_ecs::prelude::ResMut;
    use bytes::BytesMut;
    use mcrs_minecraft_protocol::Decode;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol::decode::PacketDecoder;
    use mcrs_minecraft_protocol::packets::configuration::clientbound::{
        ClientboundFinishConfiguration, ClientboundRegistryData, ClientboundSelectKnownPacks,
        ClientboundUpdateTags, RegistryTags, TagGroup,
    };
    use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundStartConfiguration;
    use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
    use mcrs_minecraft_protocol::registry::Entry;
    use mcrs_minecraft_protocol::resource_pack::KnownPack;
    use mcrs_minecraft_resource::ResourceLocation;
    use mcrs_minecraft_text::Text;
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
        app.add_message::<ConnectionEnded>();
        app.add_systems(Update, (receive_packets, flush).chain());
        add_packet_observers(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                ClientConnection::new(raw, None, PacketDispatcher::clientbound()),
                ConnectionState::Game,
                Configuration::default(),
            ))
            .id();
        (app, inbound, entity)
    }

    fn ended(app: &mut App) -> Vec<ConnectionEnd> {
        app.world_mut()
            .resource_mut::<bevy_ecs::message::Messages<ConnectionEnded>>()
            .drain()
            .map(|ended| ended.reason)
            .collect()
    }

    #[test]
    fn a_cancelled_join_ends_no_session() {
        let mut app = App::new();
        app.add_plugins(ClientNetworkPlugin {
            holds_vanilla_pack: false,
            play: None,
        });
        let (failed, joined) = oneshot::channel();
        let join = app
            .world_mut()
            .spawn((
                ClientSession,
                PendingJoin {
                    joined,
                    view_distance: 2,
                },
            ))
            .id();
        drop(failed);
        app.world_mut().write_message(CancelJoin);
        app.update();
        app.update();
        assert!(app.world().get_entity(join).is_err());
        assert!(ended(&mut app).is_empty());
    }

    #[test]
    fn a_closed_connection_ends_the_session() {
        let runtime = runtime();
        let (mut app, inbound, _) = client_app(&runtime);
        app.update();
        assert!(ended(&mut app).is_empty());
        drop(inbound);
        app.update();
        assert!(matches!(ended(&mut app)[..], [ConnectionEnd::Lost(_)]));
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
        assert!(matches!(ended(&mut app)[..], [ConnectionEnd::Kicked(_)]));
        assert!(app.world().get_entity(entity).is_err());
    }

    fn body<P: Encode>(packet: &P) -> Bytes {
        let mut body = Vec::new();
        packet.encode(&mut body).unwrap();
        body.into()
    }

    fn deliver_frame(
        runtime: &Runtime,
        inbound: &mpsc::Sender<crate::ReceivedPacket>,
        id: i32,
        payload: Bytes,
    ) {
        runtime
            .block_on(inbound.send(crate::ReceivedPacket {
                timestamp: crate::Instant::now(),
                id,
                payload,
                verdict: None,
            }))
            .unwrap();
    }

    fn deliver<P: Encode + Packet>(
        runtime: &Runtime,
        inbound: &mpsc::Sender<crate::ReceivedPacket>,
        packet: &P,
    ) {
        deliver_frame(runtime, inbound, P::ID, body(packet));
    }

    const PROBE_NAME: &str = "minecraft:test_probe";

    fn probe_data(entries: &[&'static str]) -> ClientboundRegistryData<'static> {
        ClientboundRegistryData {
            registry: ResourceLocation::read(PROBE_NAME).unwrap(),
            entries: entries
                .iter()
                .map(|entry| Entry {
                    id: ResourceLocation::read(entry).unwrap(),
                    data: None,
                })
                .collect(),
        }
    }

    fn probe_tags(tag: &'static str, members: &[i32]) -> ClientboundUpdateTags<'static> {
        ClientboundUpdateTags {
            registries: vec![RegistryTags {
                registry: ResourceLocation::read(PROBE_NAME).unwrap(),
                tags: vec![TagGroup {
                    name: ResourceLocation::read(tag).unwrap(),
                    entries: members.iter().copied().map(VarInt).collect(),
                }],
            }],
        }
    }

    #[derive(Resource, Default)]
    struct Handed {
        configurations: Vec<Configuration>,
        keep_alive_read_in: Vec<ConnectionState>,
    }

    fn accept_registries(
        event: On<RegistriesReceived>,
        mut handed: ResMut<Handed>,
        mut commands: Commands,
    ) {
        handed.configurations.push(event.configuration.clone());
        finish_configuration(commands.reborrow(), event.entity);
    }

    fn record_keep_alive(
        event: On<Received<GameKeepAlive>>,
        states: Query<&ConnectionState>,
        mut handed: ResMut<Handed>,
    ) {
        if let Ok(state) = states.get(event.entity) {
            handed.keep_alive_read_in.push(*state);
        }
    }

    struct Harness {
        app: App,
        inbound: mpsc::Sender<crate::ReceivedPacket>,
        outgoing: OutboundReceiver,
        entity: Entity,
    }

    impl Harness {
        fn new(runtime: &Runtime, state: ConnectionState) -> Self {
            let _guard = runtime.enter();
            let (raw, outgoing, inbound) = RawConnection::new_for_test_full(8);
            let mut app = App::new();
            app.add_message::<ConnectionEnded>();
            app.init_resource::<Handed>();
            app.add_systems(Update, (receive_packets, flush).chain());
            add_packet_observers(&mut app);
            app.add_observer(accept_registries);
            app.add_observer(record_keep_alive);
            let entity = app
                .world_mut()
                .spawn((
                    ClientConnection::new(raw, None, PacketDispatcher::clientbound()),
                    state,
                    Configuration::default(),
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

    #[test]
    fn the_end_of_configuration_hands_over_what_the_server_sent_in_order() {
        let runtime = runtime();
        let mut client = Harness::configuring(&runtime);
        let first = probe_data(&["minecraft:zeta", "minecraft:alpha"]);
        let second = probe_data(&["minecraft:beta"]);
        let old_tags = probe_tags("minecraft:old", &[0]);
        let new_tags = probe_tags("minecraft:t", &[2, 0]);
        client.deliver(&runtime, &first);
        client.deliver(&runtime, &second);
        client.deliver(&runtime, &old_tags);
        client.deliver(&runtime, &new_tags);
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();

        assert_eq!(
            client.handed().configurations,
            [Configuration {
                registry_data: vec![body(&first), body(&second)],
                tags: vec![body(&old_tags), body(&new_tags)],
                vanilla_known: false,
            }]
        );
        assert_eq!(client.state(), ConnectionState::Game);
        assert_eq!(client.sent_ids(), [ServerboundFinishConfiguration::ID]);
    }

    #[test]
    fn a_registry_packet_that_does_not_decode_drops_the_connection() {
        let runtime = runtime();
        let mut client = Harness::configuring(&runtime);
        deliver_frame(
            &runtime,
            &client.inbound,
            ClientboundRegistryData::ID,
            Bytes::from_static(&[0xff]),
        );
        client.app.update();
        assert!(client.dropped());
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
            (answered, client.handed().configurations[0].vanilla_known)
        };

        assert_eq!(answer(false, vec![vanilla()]), (0, false));
        assert_eq!(answer(true, vec![vanilla()]), (1, true));
        assert_eq!(answer(true, vec![other]), (0, false));
    }

    #[test]
    fn a_frame_behind_the_finish_is_read_once_the_finish_is_answered() {
        let runtime = runtime();
        let mut client = Harness::configuring(&runtime);
        client.deliver(&runtime, &probe_data(&["minecraft:a"]));
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.deliver(
            &runtime,
            &GameKeepAlive(
                mcrs_minecraft_protocol::packets::common::clientbound::KeepAlive { payload: 7 },
            ),
        );
        client.app.update();
        client.app.update();

        assert_eq!(client.handed().keep_alive_read_in, [ConnectionState::Game]);
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
        client.deliver(&runtime, &probe_data(&["minecraft:a"]));
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

        let again = probe_data(&["minecraft:c"]);
        client.deliver(&runtime, &again);
        client.deliver(&runtime, &ClientboundFinishConfiguration);
        client.app.update();
        assert_eq!(
            client.handed().configurations[1].registry_data,
            [body(&again)]
        );
        assert_eq!(client.state(), ConnectionState::Game);
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
