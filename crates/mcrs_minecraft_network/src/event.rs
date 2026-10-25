use crate::Instant;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::prelude::Commands;
use bytes::Bytes;
use mcrs_minecraft_protocol::packets::table::{Row, Visit};
use mcrs_minecraft_protocol::{Decode, Packet};
use std::marker::PhantomData;
use tracing::error;

/// A frame of one typed row, checked by the [`PacketDispatcher`](crate::metrics::PacketDispatcher)
/// of its connection. The row type names the table, so an observer of it sees only the frames of
/// that table's state and side.
///
/// Most packets borrow from their frame and an event outlives none, so the event holds the frame
/// and [`Received::packet`] decodes it once more.
#[derive(EntityEvent)]
pub struct Received<R: Row> {
    pub entity: Entity,
    pub timestamp: Instant,
    data: Bytes,
    row: PhantomData<fn() -> R>,
}

impl<R: Row> Received<R> {
    pub fn new(entity: Entity, data: Bytes, timestamp: Instant) -> Self {
        Self {
            entity,
            timestamp,
            data,
            row: PhantomData,
        }
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }

    /// `None` only for an event that was built without the dispatcher, since the dispatcher
    /// decoded the same bytes before it raised the event.
    pub fn packet(&self) -> Option<R::Packet<'_>> {
        let mut r = &self.data[..];
        match R::Packet::decode(&mut r) {
            Ok(packet) if r.is_empty() => Some(packet),
            Ok(_) => {
                error!("{}: {} bytes left over", R::Packet::NAME, r.len());
                None
            }
            Err(fault) => {
                error!("{}: {fault:#}", R::Packet::NAME);
                None
            }
        }
    }
}

impl<R: Row> std::fmt::Debug for Received<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Received")
            .field("entity", &self.entity)
            .field("row", &R::Packet::NAME)
            .field("timestamp", &self.timestamp)
            .finish_non_exhaustive()
    }
}

/// Raises the [`Received`] event of one row. It holds no data, so a checked frame can carry it
/// across a channel to another world.
#[derive(Clone, Copy)]
pub struct Trigger {
    name: &'static str,
    raise: fn(&mut Commands, Entity, Bytes, Instant),
}

impl Trigger {
    pub fn of<R: Row>() -> Self {
        Self {
            name: R::Packet::NAME,
            raise: |commands, entity, data, timestamp| {
                commands.trigger(Received::<R>::new(entity, data, timestamp));
            },
        }
    }

    pub fn raise(self, commands: &mut Commands, entity: Entity, data: Bytes, timestamp: Instant) {
        (self.raise)(commands, entity, data, timestamp);
    }
}

impl std::fmt::Debug for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Trigger").field(&self.name).finish()
    }
}

pub(crate) struct Triggers;

impl Visit for Triggers {
    type Output = Trigger;

    fn row<R: Row>(&mut self) -> Trigger {
        Trigger::of::<R>()
    }
}

#[cfg(not(target_family = "wasm"))]
mod loop_plugin {
    use crate::inbound_rate::InboundRateBucket;
    use crate::metrics::{BridgeTelemetry, PacketDispatcher};
    use crate::{ConnectionState, ServerSideConnection};
    use bevy_app::{App, Plugin, Update};
    use bevy_ecs::entity::Entity;
    use bevy_ecs::prelude::Commands;
    use bevy_ecs::system::{Query, ResMut};
    use tracing::warn;

    pub(crate) struct EventLoopPlugin;

    impl Plugin for EventLoopPlugin {
        fn build(&self, app: &mut App) {
            // The pre-game loop and the bridge share one dispatcher, whichever plugin builds first.
            app.world_mut()
                .get_resource_or_insert_with(PacketDispatcher::serverbound);
            app.init_resource::<BridgeTelemetry>();
            app.add_systems(Update, run_event_loop);
        }
    }

    // chisle: a guess above the handful of frames the reference client sends in a
    // pre-game state; the measured cost of decoding one frame sets it.
    pub const PRE_GAME_FRAMES_PER_PASS: usize = 16;

    #[cfg_attr(
        feature = "telemetry-tracy",
        tracing::instrument(name = "network::process_received_packet", skip_all)
    )]
    pub fn run_event_loop(
        mut query: Query<(
            Entity,
            &mut ServerSideConnection,
            &mut InboundRateBucket,
            &ConnectionState,
        )>,
        mut commands: Commands,
        mut dispatcher: ResMut<PacketDispatcher>,
        mut telemetry: ResMut<BridgeTelemetry>,
    ) {
        for (entity, mut conn, mut bucket, &state) in &mut query {
            // A connection in game is read by the server's bridge, which rate-limits it.
            if state == ConnectionState::Game {
                continue;
            }
            // What a pass leaves unread stays in the channel, which stops the socket reader.
            for _ in 0..PRE_GAME_FRAMES_PER_PASS {
                match conn.raw.try_recv() {
                    Ok(Some(pkt)) => {
                        if !bucket.consume_or_flag(pkt.timestamp) {
                            commands.entity(entity).remove::<ServerSideConnection>();
                            telemetry.kick_flood_total += 1;
                            break;
                        }
                        match dispatcher.dispatch(state, pkt.id, &pkt.payload) {
                            Ok(Some(trigger)) => {
                                trigger.raise(&mut commands, entity, pkt.payload, pkt.timestamp)
                            }
                            Ok(None) => {}
                            Err(malformed) => {
                                // chisle: once the server accepts a login the game disconnects
                                // in the configuration protocol, but this loop cannot see that
                                // the login was accepted; handing it the client's protocol
                                // lifts it.
                                conn.reject(state, &malformed);
                                commands.entity(entity).remove::<ServerSideConnection>();
                                break;
                            }
                        }
                        if dispatcher.ends_state(state, pkt.id) {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        warn!("disconnecting client: {e}");
                        commands.entity(entity).despawn();
                        break;
                    }
                }
            }
        }
    }
}
#[cfg(not(target_family = "wasm"))]
pub(crate) use loop_plugin::EventLoopPlugin;
#[cfg(not(target_family = "wasm"))]
pub use loop_plugin::{PRE_GAME_FRAMES_PER_PASS, run_event_loop};
