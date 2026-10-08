use crate::Instant;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bytes::Bytes;
use mcrs_minecraft_protocol::{Decode, Packet, PacketSide};
use tracing::warn;

#[derive(Debug, Clone, EntityEvent)]
pub struct ReceivedPacketEvent {
    pub entity: Entity,
    pub id: i32,
    pub data: Bytes,
    pub timestamp: Instant,
}

impl ReceivedPacketEvent {
    pub fn try_decode<'a, P>(&'a self) -> Option<anyhow::Result<P>>
    where
        P: Decode<'a> + Packet,
    {
        if self.id != P::ID {
            return None;
        }

        let mut r = &self.data[..];
        Some(match P::decode(&mut r) {
            Ok(pkt) if r.is_empty() => Ok(pkt),
            Ok(_) => Err(anyhow::anyhow!("{} bytes left over", r.len())),
            Err(error) => Err(error),
        })
    }

    #[inline]
    pub fn decode<'a, P>(&'a self) -> Option<P>
    where
        P: Decode<'a> + Packet,
    {
        let fault = match self.try_decode::<P>()? {
            Ok(packet) => return Some(packet),
            Err(fault) => fault,
        };
        // A serverbound frame was counted, and its row's first fault logged, by the loop
        // that read it off the socket.
        // chisle: nothing counts clientbound rows, so a client logs every fault of one;
        // a counter in the client's receive loop replaces this line.
        if P::SIDE == PacketSide::Clientbound {
            warn!("PacketEvent decode error: {} {fault:#}", P::NAME);
        }
        None
    }
}

#[cfg(not(target_family = "wasm"))]
mod loop_plugin {
    use super::ReceivedPacketEvent;
    use crate::inbound_rate::InboundRateBucket;
    use crate::metrics::{BridgeTelemetry, PreGameDecodeCounts};
    use crate::{ConnectionState, ServerSideConnection};
    use bevy_app::{App, Plugin, Update};
    use bevy_ecs::entity::Entity;
    use bevy_ecs::prelude::Commands;
    use bevy_ecs::system::{Query, ResMut};
    use mcrs_minecraft_protocol::Packet;
    use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
    use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundLoginAcknowledged;
    use tracing::warn;

    pub(crate) struct EventLoopPlugin;

    impl Plugin for EventLoopPlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<PreGameDecodeCounts>();
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
            Option<&ConnectionState>,
        )>,
        mut commands: Commands,
        mut counts: ResMut<PreGameDecodeCounts>,
        mut telemetry: ResMut<BridgeTelemetry>,
    ) {
        for (entity, mut conn, mut bucket, state) in &mut query {
            let ends_state = match state {
                // A connection in game is read by the server's bridge, which rate-limits it.
                Some(ConnectionState::Game) => continue,
                Some(ConnectionState::Login) => Some(ServerboundLoginAcknowledged::ID),
                Some(ConnectionState::Configuration) => Some(ServerboundFinishConfiguration::ID),
                None => None,
            };
            // What a pass leaves unread stays in the channel, which stops the socket reader.
            for _ in 0..PRE_GAME_FRAMES_PER_PASS {
                match conn.raw.try_recv() {
                    Ok(Some(pkt)) => {
                        if !bucket.consume_or_flag(pkt.timestamp) {
                            commands.entity(entity).remove::<ServerSideConnection>();
                            telemetry.kick_flood_total += 1;
                            break;
                        }
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
                        // The frames behind one that ends the state are in the next state's
                        // protocol, and the state changes only once this one is handled.
                        if ends_state == Some(pkt.id) {
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
