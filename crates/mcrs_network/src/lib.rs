pub mod connect;
pub mod event;
pub mod metrics;
mod intent;
mod packet_io;
mod status;

pub use crate::packet_io::{MAX_QUEUED_BYTES_PER_SOCKET, RawConnection};
use bevy_app::{App, FixedPreUpdate, Plugin, PostStartup};
use bevy_ecs::prelude::Component;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Res};
use bevy_ecs::world::World;

/// System sets for the network layer, usable for ordering constraints in
/// downstream crates. `SpawnConnections` contains `spawn_new_raw_connections`.
/// Other crates should schedule their connection-setup systems
/// `.after(NetworkSet::SpawnConnections)` in `FixedPreUpdate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub enum NetworkSet {
    SpawnConnections,
}
use bytes::Bytes;
use mcrs_protocol::{Encode, Packet, WritePacket};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Sender, channel};

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        build_plugin(app).expect("Failed to build network plugin");
    }
}

fn build_plugin(app: &mut App) -> anyhow::Result<()> {
    let runtime = Runtime::new()?;
    let tokio_handle = runtime.handle().clone();

    let (new_sessions_send, mut new_sessions_recv) = channel(128);

    let shared_state = SharedNetworkState(Arc::new(SharedNetworkStateInner {
        address: SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 25565).into(),
        tokio_handle,
        tokio_runtime: Some(runtime),
        new_connections_send: new_sessions_send,
    }));

    app.insert_resource(shared_state.clone());

    let start_accept_loop = move |shared_state: Res<SharedNetworkState>| {
        let _guard = shared_state.0.tokio_handle.enter();
        tokio::spawn(connect::start_accept_loop(shared_state.clone()));
    };
    let spawn_new_raw_connections = move |world: &mut World| {
        for _ in 0..new_sessions_recv.len() {
            match new_sessions_recv.try_recv() {
                Ok(session) => {
                    // OutboundQueue and InboundRateBucket components live in mcrs_minecraft
                    // and are attached via an observer in the bridge plugin, not here.
                    world.spawn((ServerSideConnection { raw: session }, ConnectionState::Login))
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
    // is registered by BridgePlugin in mcrs_minecraft.
    app.add_plugins(event::EventLoopPlugin);

    Ok(())
}

#[derive(Resource, Clone)]
struct SharedNetworkState(Arc<SharedNetworkStateInner>);

struct SharedNetworkStateInner {
    address: SocketAddr,
    tokio_handle: Handle,
    // Held to keep the runtime alive for the process lifetime; dropping it shuts down all tasks.
    #[allow(dead_code)]
    tokio_runtime: Option<Runtime>,
    new_connections_send: Sender<Box<RawConnection>>,
}

#[derive(Clone, Debug)]
pub struct ReceivedPacket {
    pub timestamp: Instant,
    pub id: i32,
    pub payload: Bytes,
}

#[derive(Component)]
pub struct ServerSideConnection {
    pub raw: Box<RawConnection>,
}

#[derive(Debug, Component, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ConnectionState {
    Login,
    Configuration,
    Game,
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

pub trait EngineConnection: Send + Sync + 'static {
    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError>;
    fn flush(&mut self) -> anyhow::Result<()>;
    fn queued_bytes(&self) -> usize;
}
