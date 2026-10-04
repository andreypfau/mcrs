use crate::Instant;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bytes::Bytes;
use mcrs_minecraft_protocol::{Decode, Packet};
use tracing::warn;

#[derive(Debug, Clone, EntityEvent)]
pub struct ReceivedPacketEvent {
    pub entity: Entity,
    pub id: i32,
    pub data: Bytes,
    pub timestamp: Instant,
}

impl ReceivedPacketEvent {
    #[inline]
    pub fn decode<'a, P>(&'a self) -> Option<P>
    where
        P: Decode<'a> + Packet,
    {
        if self.id != P::ID {
            return None;
        }

        let mut r = &self.data[..];
        match P::decode(&mut r) {
            Ok(pkt) => {
                if r.is_empty() {
                    return Some(pkt);
                }
                warn!("PacketEvent decode: {} bytes left over", r.len());
            }
            Err(e) => {
                warn!("PacketEvent decode error: {:?}", e);
            }
        }
        None
    }
}

#[cfg(not(target_family = "wasm"))]
mod loop_plugin {
    use super::ReceivedPacketEvent;
    use crate::metrics::PreGameDecodeCounts;
    use crate::{ConnectionState, ServerSideConnection};
    use bevy_app::{App, Plugin, Update};
    use bevy_ecs::entity::Entity;
    use bevy_ecs::prelude::Commands;
    use bevy_ecs::system::{Query, ResMut};
    use tracing::warn;

    pub(crate) struct EventLoopPlugin;

    impl Plugin for EventLoopPlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<PreGameDecodeCounts>();
            app.add_systems(Update, run_event_loop);
        }
    }

    #[cfg_attr(
        feature = "telemetry-tracy",
        tracing::instrument(name = "network::process_received_packet", skip_all)
    )]
    pub fn run_event_loop(
        mut query: Query<(Entity, &mut ServerSideConnection, Option<&ConnectionState>)>,
        mut commands: Commands,
        mut counts: ResMut<PreGameDecodeCounts>,
    ) {
        query.iter_mut().for_each(|(entity, mut conn, state)| {
            // A connection in game is read by the server's bridge, which rate-limits it.
            if state == Some(&ConnectionState::Game) {
                return;
            }
            loop {
                match conn.raw.try_recv() {
                    Ok(Some(pkt)) => {
                        if let Some(state) = state
                            && let Some(line) = counts.record(*state, pkt.id, &pkt.payload)
                        {
                            warn!("{line}");
                        }
                        commands.trigger(ReceivedPacketEvent {
                            entity,
                            id: pkt.id,
                            data: pkt.payload,
                            timestamp: pkt.timestamp,
                        });
                    }
                    Ok(None) => break,
                    Err(e) => {
                        warn!("disconnecting client: {e}");
                        commands.entity(entity).despawn();
                        break;
                    }
                }
            }
        });
    }
}
#[cfg(not(target_family = "wasm"))]
pub(crate) use loop_plugin::EventLoopPlugin;
#[cfg(not(target_family = "wasm"))]
pub use loop_plugin::run_event_loop;

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::metrics::PreGameDecodeCounts;
    use crate::{ConnectionState, RawConnection, ReceivedPacket, ServerSideConnection};
    use bevy_ecs::system::{IntoSystem, System};
    use bevy_ecs::world::World;
    use mcrs_minecraft_protocol::packets::table::{configuration_serverbound, login_serverbound};
    use std::sync::OnceLock;
    use tokio::sync::mpsc;

    fn spawn_connection(
        world: &mut World,
        state: ConnectionState,
    ) -> (Entity, mpsc::Sender<ReceivedPacket>) {
        static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        let runtime = RUNTIME.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap()
        });
        let (raw, _outgoing, inbound) =
            runtime.block_on(async { RawConnection::new_for_test_full(16) });
        let entity = world
            .spawn((ServerSideConnection { raw: Box::new(raw) }, state))
            .id();
        (entity, inbound)
    }

    fn frame(id: i32) -> ReceivedPacket {
        ReceivedPacket {
            timestamp: Instant::now(),
            id,
            payload: Bytes::new(),
        }
    }

    fn run(world: &mut World) {
        let mut system = IntoSystem::into_system(loop_plugin::run_event_loop);
        system.initialize(world);
        let _ = system.run((), world);
        system.apply_deferred(world);
    }

    #[test]
    fn an_unknown_id_before_the_game_state_is_counted_and_nothing_is_despawned() {
        let mut world = World::new();
        world.init_resource::<PreGameDecodeCounts>();
        let (login, login_tx) = spawn_connection(&mut world, ConnectionState::Login);
        let (configuration, configuration_tx) =
            spawn_connection(&mut world, ConnectionState::Configuration);
        login_tx
            .try_send(frame(login_serverbound::NAMES.len() as i32))
            .unwrap();
        configuration_tx
            .try_send(frame(configuration_serverbound::NAMES.len() as i32 + 3))
            .unwrap();
        configuration_tx.try_send(frame(-1)).unwrap();

        run(&mut world);

        let counts = world.resource::<PreGameDecodeCounts>();
        assert_eq!(counts.counts(ConnectionState::Login).unwrap().unknown(), 1);
        assert_eq!(
            counts
                .counts(ConnectionState::Configuration)
                .unwrap()
                .unknown(),
            2
        );
        assert!(world.entity(login).contains::<ServerSideConnection>());
        assert!(
            world
                .entity(configuration)
                .contains::<ServerSideConnection>()
        );
    }
}
