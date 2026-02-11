use crate::{EngineConnection, ServerSideConnection};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::prelude::Commands;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::{ParallelCommands, Query};
use bytes::Bytes;
use log::{debug, info, warn};
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
                eprintln!("PacketEvent decode: {} bytes left over", r.len());
            }
            Err(e) => {
                eprintln!("PacketEvent decode error: {:?}", e);
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

fn run_event_loop(mut query: Query<(Entity, &mut ServerSideConnection)>, mut commands: Commands) {
    query.iter_mut().for_each(|(entity, mut conn)| {
        loop {
            match conn.try_recv() {
                Ok(Some(pkt)) => {
                    let now = Instant::now();
                    let process_latency = now - pkt.timestamp;
                    let _span = tracing::info_span!("process_received_packet").entered();
                    commands.trigger(ReceivedPacketEvent {
                        entity,
                        id: pkt.id,
                        data: pkt.payload,
                        timestamp: pkt.timestamp,
                    });
                    // info!(
                    //     "{}: processed packet {} in {:?}",
                    //     entity, pkt.id, process_latency
                    // );
                }
                Ok(None) => break,
                Err(e) => {
                    warn!("disconnecting client: {e}");
                    commands.entity(entity).remove::<ServerSideConnection>();
                    break;
                }
            }
        }
    });
}
