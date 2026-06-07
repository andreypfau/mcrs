use crate::{EngineConnection, InGameConnectionState, ServerSideConnection};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::prelude::Commands;
use bevy_ecs::query::Without;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::Query;
use bytes::Bytes;
use log::warn;
use mcrs_protocol::{Decode, Packet};
use std::time::Instant;

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

pub(crate) struct EventLoopPlugin;

impl Plugin for EventLoopPlugin {
    fn build(&self, app: &mut App) {
        // app.init_schedule(RunEventLoop);
        // let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        app.add_systems(Update, run_event_loop);
    }
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RunEventLoop;

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "network::process_received_packet", skip_all)
)]
fn run_event_loop(
    mut query: Query<(Entity, &mut ServerSideConnection), Without<InGameConnectionState>>,
    mut commands: Commands,
) {
    query.iter_mut().for_each(|(entity, mut conn)| {
        loop {
            match conn.try_recv() {
                Ok(Some(pkt)) => {
                    commands.trigger(ReceivedPacketEvent {
                        entity,
                        id: pkt.id,
                        data: pkt.payload,
                        timestamp: pkt.timestamp,
                    });
                    // let now = Instant::now();
                    // info!(
                    //     "{}: processed packet {} in {:?}",
                    //     entity,
                    //     pkt.id,
                    //     now - pkt.timestamp
                    // );
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
