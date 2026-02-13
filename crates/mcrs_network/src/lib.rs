#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    unused_mut,
    unused_parens,
    unreachable_pub,
    clippy::uninlined_format_args
)]

pub mod connect;
pub mod event;
mod intent;
mod packet_io;
mod status;

use crate::packet_io::RawConnection;
use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin, PostStartup};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Component;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Query, Res};
use bevy_ecs::world::World;
use bytes::Bytes;
use log::warn;
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
                Ok((session)) => {
                    let connection = ServerSideConnection { raw: session };
                    world.spawn((connection, ConnectionState::Login))
                }
                Err(_) => break,
            };
        }
    };

    app.add_systems(PostStartup, start_accept_loop);
    app.add_systems(FixedPreUpdate, spawn_new_raw_connections);
    app.add_systems(FixedPostUpdate, (flush_packets, check_congestion).chain());
    app.add_plugins(event::EventLoopPlugin);

    Ok(())
}

fn flush_packets(
    mut connections: Query<(Entity, &mut ServerSideConnection)>,
    mut commands: Commands,
) {
    for (entity, mut connection) in connections.iter_mut() {
        if let Err(e) = connection.flush() {
            commands.entity(entity).despawn();
            warn!("Connection to {} closed: {}", connection.remote_addr(), e);
        }
    }
}

/// 32 MiB hard limit on queued outgoing bytes before disconnecting.
/// Must accommodate initial join burst (chunks + entity data).
const MAX_QUEUED_BYTES: usize = 32 * 1024 * 1024;

fn check_congestion(connections: Query<(Entity, &ServerSideConnection)>, mut commands: Commands) {
    for (entity, connection) in connections.iter() {
        let queued = connection.queued_bytes();
        if queued > MAX_QUEUED_BYTES {
            warn!(
                "Connection to {} congested ({} bytes queued), disconnecting",
                connection.remote_addr(),
                queued
            );
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Resource, Clone)]
struct SharedNetworkState(Arc<SharedNetworkStateInner>);

struct SharedNetworkStateInner {
    address: SocketAddr,
    tokio_handle: Handle,
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
    raw: Box<RawConnection>,
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
        _ = self.flush()
    }
}

pub trait EngineConnection: Send + Sync + 'static {
    /// Receives the next pending serverbound packet. This must return
    /// immediately without blocking.
    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError>;

    /// Flushes encoded packets to the outgoing channel.
    /// Only fails if the connection is closed (writer task died).
    fn flush(&mut self) -> anyhow::Result<()>;

    /// Returns the number of bytes currently queued for sending.
    fn queued_bytes(&self) -> usize;
}
