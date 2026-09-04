use crate::{ConnectionState, EngineConnection, NetworkSet, RawConnection, ReceivedPacket};
use crate::{connect, event, webtransport};
use bevy_app::{App, FixedPreUpdate, Plugin, PostStartup};
use bevy_ecs::prelude::Component;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_protocol::{Encode, Packet, WritePacket};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Sender, channel};

pub struct NetworkPlugin {
    /// Port 0 asks the OS for a free port; read the result back from
    /// [`BoundAddress`].
    pub address: SocketAddr,
}

impl Default for NetworkPlugin {
    fn default() -> Self {
        Self {
            address: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 25565).into(),
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
        build_plugin(app, self.address).expect("Failed to build network plugin");
    }
}

fn build_plugin(app: &mut App, address: SocketAddr) -> anyhow::Result<()> {
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

    let shared_state = SharedNetworkState(Arc::new(SharedNetworkStateInner {
        tokio_handle,
        tokio_runtime: Some(runtime),
        new_connections_send: new_sessions_send,
    }));

    app.insert_resource(shared_state.clone());
    app.insert_resource(BoundAddress(bound));

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
        tokio::spawn(connect::start_accept_loop(shared_state.clone(), listener));
    };
    let spawn_new_raw_connections = move |world: &mut World| {
        for _ in 0..new_sessions_recv.len() {
            match new_sessions_recv.try_recv() {
                Ok(session) => {
                    // OutboundQueue and InboundRateBucket components live in mcrs_minecraft_server
                    // and are attached via an observer in the bridge plugin, not here.
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

#[derive(Resource, Clone)]
pub(crate) struct SharedNetworkState(pub(crate) Arc<SharedNetworkStateInner>);

pub(crate) struct SharedNetworkStateInner {
    pub(crate) tokio_handle: Handle,
    // Held to keep the runtime alive for the process lifetime; dropping it shuts down all tasks.
    #[allow(dead_code)]
    tokio_runtime: Option<Runtime>,
    pub(crate) new_connections_send: Sender<Box<RawConnection>>,
}

#[derive(Component)]
pub struct ServerSideConnection {
    pub raw: Box<RawConnection>,
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct InGameConnectionState;

impl ServerSideConnection {
    pub fn remote_addr(&self) -> SocketAddr {
        self.raw.remote_addr
    }

    pub fn queued_bytes(&self) -> usize {
        self.raw.queued_bytes()
    }
}

impl WritePacket for ServerSideConnection {
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

impl EngineConnection for ServerSideConnection {
    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError> {
        self.raw.try_recv()
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        self.raw.flush()
    }

    fn queued_bytes(&self) -> usize {
        self.raw.queued_bytes()
    }
}

impl Drop for ServerSideConnection {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
