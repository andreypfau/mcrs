use crate::inbound_rate::InboundRateBucket;
use crate::intent::status_frame;
use crate::metrics::Malformed;
use crate::{ConnectionState, NetworkSet, RawConnection};
use crate::{connect, event, lan, webtransport};
use base64::engine::general_purpose::STANDARD;
use bevy_app::{App, AppExit, FixedPreUpdate, Plugin, PostStartup, Update};
use bevy_ecs::change_detection::{DetectChanges, ResMut};
use bevy_ecs::prelude::Component;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use bytes::Bytes;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::packets::{configuration, game};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Encode, Packet, WritePacket};
use mcrs_minecraft_text::Text;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::{Sender, channel};
use tokio::sync::watch;
use tracing::{info, warn};

pub struct NetworkPlugin {
    /// Port 0 asks the OS for a free port; read the result back from
    /// [`BoundAddress`].
    pub address: SocketAddr,
    /// A listener beyond loopback announces itself on the local network.
    pub announce_on_lan: bool,
    pub max_players: u32,
    pub motd: String,
    pub favicon: Option<String>,
    /// A handshake with the transfer intent continues as a login only when this is set;
    /// otherwise the client gets `multiplayer.disconnect.transfers_disabled`.
    pub accepts_transfers: bool,
    /// Turned on, the status response keeps the online and maximum counts but carries no sample
    /// of player names.
    pub hide_online_players: bool,
    /// Turned off, a status connection closes after the handshake without a packet; a login is
    /// not affected.
    pub enable_status: bool,
}

const SERVER_ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/server-icon.png"
));

/// The mc.rs icon as a `data:` URL for the `favicon` key of the status response.
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
        }
    }
}

/// The address the listener is actually bound to, inserted while the plugin
/// builds, so a caller that asked for port 0 can read the port before the app
/// ever ticks.
#[derive(Resource, Clone, Copy, Debug)]
pub struct BoundAddress(pub SocketAddr);

/// The UDP address the WebTransport endpoint is bound to, plus the SHA-256 of
/// the DER certificate it minted at startup. A browser can only reach a
/// self-signed endpoint by passing that hash in `serverCertificateHashes`, and
/// only accepts one whose certificate is valid for at most 14 days, so the
/// certificate is ephemeral and its hash has to travel out of band.
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
    /// Replaces the online players; the next status exchange sees them. A system that calls this
    /// must run before [`NetworkSet::RefreshStatus`] to show the change in the same frame.
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

fn build_plugin(app: &mut App, plugin: &NetworkPlugin) -> anyhow::Result<()> {
    let address = plugin.address;
    let announce_on_lan = plugin.announce_on_lan;
    let runtime = Runtime::new()?;
    let tokio_handle = runtime.handle().clone();

    // Binding through std keeps this usable from a caller that is itself
    // already inside a tokio runtime, where `block_on` would panic.
    let std_listener = std::net::TcpListener::bind(address)?;
    std_listener.set_nonblocking(true)?;
    let bound = std_listener.local_addr()?;
    let listener = {
        let _guard = tokio_handle.enter();
        tokio::net::TcpListener::from_std(std_listener)?
    };

    let (new_sessions_send, mut new_sessions_recv) = channel(128);
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
    }));
    status_send.send_replace(status_frame(&StatusPlayers::default(), &shared_state.0)?);

    app.insert_resource(shared_state.clone());
    app.insert_resource(BoundAddress(bound));
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
    let spawn_new_raw_connections = move |world: &mut World| {
        for _ in 0..new_sessions_recv.len() {
            match new_sessions_recv.try_recv() {
                Ok(session) => {
                    // The outbound queue lives in mcrs_minecraft_server, which attaches it.
                    world.spawn((
                        ServerSideConnection { raw: session },
                        ConnectionState::Login,
                    ))
                }
                Err(_) => break,
            };
        }
    };

    app.add_systems(PostStartup, start_accept_loop);
    app.configure_sets(FixedPreUpdate, NetworkSet::SpawnConnections);
    app.add_systems(
        FixedPreUpdate,
        spawn_new_raw_connections.in_set(NetworkSet::SpawnConnections),
    );
    // flush_packets and check_congestion removed; the FixedPostUpdate bridge chain
    // is registered by BridgePlugin in mcrs_minecraft_server.
    app.add_plugins(event::EventLoopPlugin);

    Ok(())
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

fn status_due(publisher: Res<StatusPublisher>, clock: Res<StatusClock>) -> bool {
    publisher.is_changed() || clock.built_at.elapsed() >= STATUS_PERIOD
}

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
}

#[derive(Component)]
#[require(InboundRateBucket)]
pub struct ServerSideConnection {
    pub raw: Box<RawConnection>,
}

impl ServerSideConnection {
    pub fn remote_addr(&self) -> SocketAddr {
        self.raw.remote_addr
    }

    /// Sends the disconnect packet of the protocol the client decodes in `state`, behind
    /// what is already written; the caller then removes the connection. While the client
    /// reads a layer the server does not yet write, the connection closes with no packet.
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
            ConnectionState::Game => self
                .raw
                .append(&game::clientbound::ClientboundDisconnect { reason })?,
            // Neither protocol has a disconnect packet; the connection only closes.
            ConnectionState::Handshaking | ConnectionState::Status => return Ok(()),
        }
        let blob = self.raw.take_encoded();
        self.raw.try_send_blob(blob);
        Ok(())
    }

    /// Logs a frame the game's decoder rejects and disconnects its client as the game does.
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
