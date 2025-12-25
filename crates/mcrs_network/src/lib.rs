mod byte_channel;
pub mod connect;
pub mod event;
mod intent;
mod packet_io;
mod status;

use crate::byte_channel::TrySendError;
use crate::packet_io::RawConnection;
use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin, PostStartup};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Component;
use bevy_ecs::query::Changed;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Query, Res};
use bevy_ecs::world::World;
use bytes::{Bytes, BytesMut};
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

    let (new_sessions_send, mut new_sessions_recv) = channel(1);

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
    app.add_systems(FixedPostUpdate, flush_packets);
    app.add_plugins(event::EventLoopPlugin);

    Ok(())
}

fn flush_packets(
    mut connections: Query<(Entity, &mut ServerSideConnection), Changed<ServerSideConnection>>,
    mut commands: Commands,
) {
    for (entity, mut connection) in connections.iter_mut() {
        if let Err(e) = connection.flush() {
            commands.entity(entity).despawn();
            eprintln!("Connection to {} closed: {}", connection.remote_addr(), e);
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

impl ServerSideConnection {
    pub fn remote_addr(&self) -> SocketAddr {
        self.raw.remote_addr
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
    fn try_send(&mut self, bytes: BytesMut) -> Result<(), TrySendError> {
        self.raw.try_send(bytes)
    }

    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError> {
        self.raw.try_recv()
    }

    fn flush(&mut self) -> Result<(), TrySendError> {
        self.raw.flush()
    }
}

impl Drop for ServerSideConnection {
    fn drop(&mut self) {
        _ = self.flush()
    }
}

pub trait EngineConnection: Send + Sync + 'static {
    /// Sends encoded clientbound packet data. This function must not block and
    /// the data should be sent as soon as possible.
    fn try_send(&mut self, bytes: BytesMut) -> Result<(), TrySendError>;
    /// Receives the next pending serverbound packet. This must return
    /// immediately without blocking.
    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError>;

    fn flush(&mut self) -> Result<(), TrySendError>;
}
