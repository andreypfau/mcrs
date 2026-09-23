use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Query, Res};
use bevy_ecs::query::Changed;
use bevy_time::{Real, Time};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundKeepAlive as ConfigurationRequest;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundKeepAlive as ConfigurationResponse;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundKeepAlive as GameRequest;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive as GameResponse;
use std::time::Duration;
use tracing::{debug, warn};

/// The reference's `KEEPALIVE_LIMIT`: how long after the last challenge the next one is due,
/// and how long a challenge may go unanswered.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

pub struct KeepAlivePlugin;

impl Plugin for KeepAlivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(bevy_app::FixedPreUpdate, handle_keepalive);
        app.add_systems(bevy_app::FixedPreUpdate, new_connection);
        app.add_observer(handle_keepalive_response);
    }
}

/// Times are the real-time clock's elapsed time: a keep-alive measures the wire, not the tick.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepAlive {
    Idle { since: Duration },
    Awaiting { challenge: i64, sent: Duration },
}

/// Each phase starts its own keep-alive, as the reference's packet listener for each phase does.
pub fn new_connection(
    query: Query<(Entity, &ConnectionState), Changed<ConnectionState>>,
    time: Res<Time<Real>>,
    mut commands: Commands,
) {
    for (entity, state) in query {
        if *state == ConnectionState::Login {
            commands.entity(entity).remove::<KeepAlive>();
            continue;
        }
        commands.entity(entity).insert(KeepAlive::Idle {
            since: time.elapsed(),
        });
    }
}

pub fn handle_keepalive(
    mut query: Query<(
        Entity,
        &mut ServerSideConnection,
        &ConnectionState,
        &mut KeepAlive,
    )>,
    time: Res<Time<Real>>,
    mut commands: Commands,
) {
    let now = time.elapsed();
    for (entity, mut con, conn_state, mut state) in query.iter_mut() {
        match *state {
            KeepAlive::Awaiting { sent, .. } if now.saturating_sub(sent) >= KEEPALIVE_INTERVAL => {
                warn!("Keepalive timeout for {}", con.remote_addr());
                commands.entity(entity).remove::<ServerSideConnection>();
            }
            KeepAlive::Idle { since } if now.saturating_sub(since) >= KEEPALIVE_INTERVAL => {
                let challenge = rand::random();
                debug!(
                    "Sending keepalive to {} with payload {}",
                    con.remote_addr(),
                    challenge
                );
                let request = mcrs_minecraft_protocol::packets::common::clientbound::KeepAlive {
                    payload: challenge,
                };
                match conn_state {
                    ConnectionState::Configuration => {
                        con.write_packet(&ConfigurationRequest(request))
                    }
                    ConnectionState::Game => con.write_packet(&GameRequest(request)),
                    ConnectionState::Login => continue,
                }
                *state = KeepAlive::Awaiting {
                    challenge,
                    sent: now,
                };
            }
            _ => {}
        }
    }
}

pub fn handle_keepalive_response(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(&ServerSideConnection, &ConnectionState, &mut KeepAlive)>,
    mut commands: Commands,
) {
    let Ok((con, conn_state, mut state)) = query.get_mut(event.entity) else {
        return;
    };
    let keep_alive = match conn_state {
        ConnectionState::Configuration => {
            let Some(pkt) = event.decode::<ConfigurationResponse>() else {
                return;
            };
            pkt.0
        }
        ConnectionState::Game => {
            let Some(pkt) = event.decode::<GameResponse>() else {
                return;
            };
            pkt.0
        }
        _ => return,
    };
    debug!("Keepalive response: {:?}", keep_alive);
    match *state {
        KeepAlive::Awaiting { challenge, sent } if keep_alive.payload == challenge => {
            *state = KeepAlive::Idle { since: sent };
        }
        _ => {
            warn!(
                "Keepalive failed for {}: got {} while {:?}",
                con.remote_addr(),
                keep_alive.payload,
                *state
            );
            commands
                .entity(event.entity)
                .remove::<ServerSideConnection>();
        }
    }
}
