use crate::Instant;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::prelude::Commands;
use bytes::Bytes;
use mcrs_minecraft_protocol::packets::table::{Row, Visit};
use mcrs_minecraft_protocol::{Decode, Packet};
use std::marker::PhantomData;
use tracing::error;

#[derive(EntityEvent)]
pub struct Received<R: Row> {
    pub entity: Entity,
    data: Bytes,
    arrival: Option<Instant>,
    row: PhantomData<fn() -> R>,
}

impl<R: Row> Received<R> {
    pub fn new(entity: Entity, data: Bytes) -> Self {
        Self {
            entity,
            data,
            arrival: None,
            row: PhantomData,
        }
    }

    pub fn arrived(entity: Entity, data: Bytes, arrival: Instant) -> Self {
        Self {
            arrival: Some(arrival),
            ..Self::new(entity, data)
        }
    }

    pub fn arrival(&self) -> Option<Instant> {
        self.arrival
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }

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
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
pub struct Trigger {
    name: &'static str,
    raise: fn(&mut Commands, Entity, Bytes, Option<Instant>),
}

impl Trigger {
    pub fn of<R: Row>() -> Self {
        Self {
            name: R::Packet::NAME,
            raise: |commands, entity, data, arrival| {
                commands.trigger(Received::<R> {
                    entity,
                    data,
                    arrival,
                    row: PhantomData,
                });
            },
        }
    }

    pub fn raise(self, mut commands: Commands, entity: Entity, data: Bytes) {
        (self.raise)(&mut commands, entity, data, None);
    }

    pub fn raise_arrived(
        self,
        mut commands: Commands,
        entity: Entity,
        data: Bytes,
        arrival: Instant,
    ) {
        (self.raise)(&mut commands, entity, data, Some(arrival));
    }
}

impl std::fmt::Debug for Trigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Trigger").field(&self.name).finish()
    }
}

pub struct Triggers;

impl Visit for Triggers {
    type Output = Trigger;

    fn row<R: Row>(&mut self) -> Trigger {
        Trigger::of::<R>()
    }
}

#[cfg(not(target_family = "wasm"))]
mod loop_plugin {
    use crate::inbound_rate::{InboundPacketRate, InboundRateLimit};
    use crate::metrics::{BridgeTelemetry, PacketDispatcher};
    use crate::{ConnectionState, ServerSideConnection};
    use bevy_app::{App, Plugin, Update};
    use bevy_ecs::entity::Entity;
    use bevy_ecs::prelude::Commands;
    use bevy_ecs::system::{Populated, Res, ResMut};
    use tracing::warn;

    pub(crate) struct EventLoopPlugin;

    impl Plugin for EventLoopPlugin {
        fn build(&self, app: &mut App) {
            app.world_mut()
                .get_resource_or_insert_with(PacketDispatcher::serverbound);
            app.init_resource::<BridgeTelemetry>();
            app.add_systems(Update, run_event_loop);
        }
    }

    pub const PRE_GAME_FRAMES_PER_PASS: usize = 16;

    #[cfg_attr(
        feature = "telemetry-tracy",
        tracing::instrument(name = "network::process_received_packet", skip_all)
    )]
    #[cfg_attr(
        mcrs_lint,
        expect(
            mcrs::blocking_in_system,
            reason = "the hand-off stamp measures wall-clock time for the stalled-writer check, which must not depend on the game clock"
        )
    )]
    #[cfg_attr(
        mcrs_lint,
        expect(
            mcrs::alloc_in_tick,
            reason = "the sites build the disconnect text of a malformed packet, so a tick allocates only when a peer sends one and then drops its connection"
        )
    )]
    #[cfg_attr(mcrs_lint, mcrs::alloc_sites(15))]
    pub fn run_event_loop(
        mut query: Populated<(
            Entity,
            &mut ServerSideConnection,
            &mut InboundPacketRate,
            &ConnectionState,
        )>,
        rate_limit: Option<Res<InboundRateLimit>>,
        mut commands: Commands,
        mut dispatcher: ResMut<PacketDispatcher>,
        mut telemetry: ResMut<BridgeTelemetry>,
    ) {
        let rate_limit = rate_limit.as_deref().copied().unwrap_or_default();
        for (entity, mut conn, mut rate, &state) in &mut query {
            if state == ConnectionState::Game {
                continue;
            }
            for _ in 0..PRE_GAME_FRAMES_PER_PASS {
                match conn.raw.try_recv() {
                    Ok(Some(pkt)) => {
                        if !rate.count(pkt.timestamp, rate_limit) {
                            conn.disconnect(state, InboundRateLimit::reason()).ok();
                            commands.entity(entity).remove::<ServerSideConnection>();
                            telemetry.kick_flood_total += 1;
                            break;
                        }
                        match dispatcher.dispatch(state, pkt.id, &pkt.payload) {
                            Ok(Some(trigger)) => trigger.raise_arrived(
                                commands.reborrow(),
                                entity,
                                pkt.payload,
                                pkt.timestamp,
                            ),
                            Ok(None) => {}
                            Err(malformed) => {
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
