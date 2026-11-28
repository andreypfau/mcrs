use crate::inbound_rate::{InboundPacketRate, InboundRateLimit};
use crate::intent::status_frame;
use crate::metrics::{Malformed, PacketDispatcher};
use crate::{ConnectionState, NetworkSet, RawConnection, WriterWakes};
use crate::{connect, event, lan, webtransport};
use base64::engine::general_purpose::STANDARD;
use bevy_app::{App, AppExit, FixedPreUpdate, Plugin, PostStartup, Update};
use bevy_ecs::change_detection::{DetectChanges, ResMut};
use bevy_ecs::prelude::Component;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Res;
use bevy_ecs::world::{FromWorld, Mut, World};
use bytes::Bytes;
use mcrs_minecraft_nbt::NbtBudget;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::packets::{configuration, game};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Encode, Packet, WritePacket};
use mcrs_minecraft_text::Text;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::watch;
use tracing::{info, warn};

pub struct NetworkPlugin {
    pub address: SocketAddr,
    pub announce_on_lan: bool,
    pub max_players: u32,
    pub motd: String,
    pub favicon: Option<String>,
    pub accepts_transfers: bool,
    pub hide_online_players: bool,
    pub enable_status: bool,
    pub send_buffer_bytes: usize,
    pub accept_backlog: u32,
    pub rate_limit: InboundRateLimit,
    pub nbt_budget: NbtBudget,
    pub per_ip_accept_limit: bool,
    pub proxy_upstreams: Vec<IpAddr>,
}

const SERVER_ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/server-icon.png"
));

#[must_use]
pub fn default_favicon() -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", STANDARD.encode(SERVER_ICON))
}

impl Default for NetworkPlugin {
    fn default() -> Self {
        Self {
            address: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 25565).into(),
            announce_on_lan: true,
            max_players: 20,
            motd: "mcrs Server".to_owned(),
            favicon: Some(default_favicon()),
            accepts_transfers: false,
            hide_online_players: false,
            enable_status: true,
            send_buffer_bytes: 512 * 1024,
            accept_backlog: 1024,
            rate_limit: InboundRateLimit::default(),
            nbt_budget: NbtBudget::default(),
            per_ip_accept_limit: true,
            proxy_upstreams: Vec::new(),
        }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct BoundAddress(pub SocketAddr);

#[derive(Resource, Clone, Copy, Debug)]
pub struct WebTransportEndpoint {
    pub address: SocketAddr,
    pub certificate_hash: [u8; 32],
}

#[derive(Clone, Default)]
pub struct StatusPlayers {
    pub players: Vec<(Uuid, String)>,
}

#[derive(Resource)]
pub struct StatusPublisher {
    sender: watch::Sender<Bytes>,
    players: StatusPlayers,
}

impl StatusPublisher {
    pub fn publish(&mut self, players: StatusPlayers) {
        self.players = players;
    }
}

#[derive(Resource)]
struct StatusClock {
    built_at: Instant,
}

const STATUS_PERIOD: Duration = Duration::from_secs(5);
impl WebTransportEndpoint {
    pub fn certificate_hash_hex(&self) -> String {
        self.certificate_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = build_plugin(app, self) {
            tracing::error!("the network did not start: {error:#}");
            app.world_mut().write_message(AppExit::error());
        }
    }
}

fn bind_listener(address: SocketAddr, backlog: u32) -> std::io::Result<std::net::TcpListener> {
    let socket = Socket::new(
        Domain::for_address(address),
        Type::STREAM,
        Some(Protocol::TCP),
    )?;
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    socket.bind(&address.into())?;
    socket.listen(i32::try_from(backlog).unwrap_or(i32::MAX))?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

fn build_plugin(app: &mut App, plugin: &NetworkPlugin) -> anyhow::Result<()> {
    let address = plugin.address;
    let announce_on_lan = plugin.announce_on_lan;
    let runtime = Runtime::new()?;
    let tokio_handle = runtime.handle().clone();

    let std_listener = bind_listener(address, plugin.accept_backlog)?;
    let bound = std_listener.local_addr()?;
    let listener = {
        let _guard = tokio_handle.enter();
        tokio::net::TcpListener::from_std(std_listener)?
    };

    let (new_sessions_send, new_sessions_recv) = channel(128);
    let (status_send, status_recv) = tokio::sync::watch::channel(Bytes::new());

    let shared_state = SharedNetworkState(Arc::new(SharedNetworkStateInner {
        tokio_handle,
        tokio_runtime: Some(runtime),
        new_connections_send: new_sessions_send,
        max_players: plugin.max_players,
        status_recv,
        motd: plugin.motd.clone(),
        favicon: plugin.favicon.clone(),
        accepts_transfers: plugin.accepts_transfers,
        hide_online_players: plugin.hide_online_players,
        enable_status: plugin.enable_status,
        send_buffer_bytes: plugin.send_buffer_bytes,
        per_ip_accept_limit: plugin.per_ip_accept_limit,
        proxy_upstreams: plugin
            .proxy_upstreams
            .iter()
            .map(IpAddr::to_canonical)
            .collect(),
    }));
    status_send.send_replace(status_frame(&StatusPlayers::default(), &shared_state.0)?);

    app.insert_resource(shared_state.clone());
    app.insert_resource(BoundAddress(bound));
    app.insert_resource(plugin.rate_limit);
    insert_status(app, status_send);

    let mut webtransport_endpoint = {
        let _guard = shared_state.0.tokio_handle.enter();
        let (endpoint, bound, certificate_hash) = webtransport::bind(address)?;
        app.insert_resource(WebTransportEndpoint {
            address: bound,
            certificate_hash,
        });
        Some(endpoint)
    };

    let mut listener = Some(listener);
    let start_accept_loop = move |shared_state: Res<SharedNetworkState>| {
        let _guard = shared_state.0.tokio_handle.enter();
        if let Some(endpoint) = webtransport_endpoint.take() {
            tokio::spawn(webtransport::start_accept_loop(
                shared_state.clone(),
                endpoint,
            ));
        }
        let Some(listener) = listener.take() else {
            return;
        };
        match lan::announce_port(bound, announce_on_lan) {
            Ok(port) => {
                let started = lan::start(
                    lan::source_address(bound),
                    lan::GROUP.into(),
                    lan::INTERVAL,
                    shared_state.0.motd.clone(),
                    port,
                    shared_state.0.new_connections_send.clone(),
                );
                if let Err(error) = started {
                    warn!("LAN announcement not started: {error}");
                }
            }
            Err(lan::Unannounced::NoIpv4) => info!(
                "LAN announcement not started: the server listens on {bound}, which accepts no \
                 IPv4, and a LAN client joins the IPv4 address the announcement comes from"
            ),
            Err(lan::Unannounced::Off | lan::Unannounced::Loopback) => {}
        }
        tokio::spawn(connect::start_accept_loop(shared_state.clone(), listener));
    };
    app.add_systems(PostStartup, start_accept_loop);
    add_connection_spawner(app, new_sessions_recv);
    app.add_plugins(event::EventLoopPlugin);
    let mut dispatcher = app.world_mut().resource_mut::<PacketDispatcher>();
    dispatcher.set_nbt_budget(plugin.nbt_budget);

    Ok(())
}

#[derive(Resource)]
struct NewConnections(Receiver<Box<RawConnection>>);

fn add_connection_spawner(app: &mut App, new_sessions_recv: Receiver<Box<RawConnection>>) {
    app.insert_resource(NewConnections(new_sessions_recv));
    app.configure_sets(FixedPreUpdate, NetworkSet::SpawnConnections);
    app.add_systems(
        FixedPreUpdate,
        spawn_new_raw_connections
            .run_if(connections_pending)
            .in_set(NetworkSet::SpawnConnections),
    );
}

fn connections_pending(new: Res<NewConnections>) -> bool {
    !new.0.is_empty()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::exclusive_system,
        reason = "spawns the accepted connections at once, so the systems later in `FixedPreUpdate` see them in the same tick"
    )
)]
fn spawn_new_raw_connections(world: &mut World) {
    world.try_resource_scope(|world, mut new: Mut<NewConnections>| {
        for _ in 0..new.0.len() {
            match new.0.try_recv() {
                Ok(session) => world.spawn((
                    ServerSideConnection { raw: session },
                    ConnectionState::Login,
                )),
                Err(_) => break,
            };
        }
    });
}

fn insert_status(app: &mut App, sender: watch::Sender<Bytes>) {
    app.insert_resource(StatusPublisher {
        sender,
        players: StatusPlayers::default(),
    });
    app.insert_resource(StatusClock {
        built_at: Instant::now(),
    });
    app.add_systems(
        Update,
        refresh_status
            .run_if(status_due)
            .in_set(NetworkSet::RefreshStatus),
    );
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn status_due(publisher: Res<StatusPublisher>, clock: Res<StatusClock>) -> bool {
    publisher.is_changed() || clock.built_at.elapsed() >= STATUS_PERIOD
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        mcrs::missed_set_if_neq,
        reason = "the status period counts wall-clock time for the server list, and each refresh stores a new instant"
    )
)]
fn refresh_status(
    publisher: Res<StatusPublisher>,
    mut clock: ResMut<StatusClock>,
    shared: Res<SharedNetworkState>,
) {
    clock.built_at = Instant::now();
    match status_frame(&publisher.players, &shared.0) {
        Ok(frame) => {
            publisher.sender.send_replace(frame);
        }
        Err(error) => warn!("the status response was not rebuilt: {error:#}"),
    }
}

#[derive(Resource, Clone)]
pub(crate) struct SharedNetworkState(pub(crate) Arc<SharedNetworkStateInner>);

pub(crate) struct SharedNetworkStateInner {
    pub(crate) tokio_handle: Handle,
    #[expect(
        dead_code,
        reason = "the runtime is held only so dropping it does not shut down the server's tasks, and nothing reads it"
    )]
    tokio_runtime: Option<Runtime>,
    pub(crate) new_connections_send: Sender<Box<RawConnection>>,
    pub(crate) max_players: u32,
    pub(crate) status_recv: tokio::sync::watch::Receiver<Bytes>,
    pub(crate) motd: String,
    pub(crate) favicon: Option<String>,
    pub(crate) accepts_transfers: bool,
    pub(crate) hide_online_players: bool,
    pub(crate) enable_status: bool,
    pub(crate) send_buffer_bytes: usize,
    pub(crate) per_ip_accept_limit: bool,
    pub(crate) proxy_upstreams: Box<[IpAddr]>,
}

impl FromWorld for WriterWakes {
    fn from_world(world: &mut World) -> Self {
        match world.get_resource::<SharedNetworkState>() {
            Some(shared) => Self::on_runtime(shared.0.tokio_handle.clone()),
            None => Self::on_this_thread(),
        }
    }
}

#[derive(Component)]
#[require(InboundPacketRate)]
pub struct ServerSideConnection {
    pub raw: Box<RawConnection>,
}

impl ServerSideConnection {
    pub fn remote_addr(&self) -> SocketAddr {
        self.raw.remote_addr
    }

    pub fn disconnect(&mut self, state: ConnectionState, reason: Text) -> anyhow::Result<()> {
        if self.raw.awaits_login_acknowledgement() {
            return Ok(());
        }
        match state {
            ConnectionState::Login => {
                let reason = serde_json::to_string(&reason)?;
                self.raw.append(&ClientboundLoginDisconnect {
                    reason: Bounded(&reason),
                })?;
            }
            ConnectionState::Configuration => self
                .raw
                .append(&configuration::clientbound::ClientboundDisconnect { reason })?,
            ConnectionState::Game => {
                self.raw.fence();
                self.raw
                    .append(&game::clientbound::ClientboundDisconnect { reason })?;
            }
            ConnectionState::Handshaking | ConnectionState::Status => return Ok(()),
        }
        self.raw.send_written();
        Ok(())
    }

    pub fn reject(&mut self, state: ConnectionState, malformed: &Malformed) {
        if let Some(line) = &malformed.log {
            warn!("{line}");
        }
        if let Err(error) = self.disconnect(state, malformed.reason()) {
            warn!("a disconnect could not be encoded: {error:#}");
        }
    }
}

impl WritePacket for ServerSideConnection {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Encode + Packet,
    {
        self.raw.write_packet_fallible(packet)
    }
}

impl Drop for ServerSideConnection {
    fn drop(&mut self) {
        let _ = self.raw.flush();
    }
}
