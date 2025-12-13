use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Query};
use bevy_ecs::query::Changed;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{ConnectionState, ServerSideConnection};
use mcrs_protocol::WritePacket;
use mcrs_protocol::packets::configuration::clientbound::ClientboundKeepAlive as ConfigurationRequest;
use mcrs_protocol::packets::configuration::serverbound::ServerboundKeepAlive as ConfigurationResponse;
use mcrs_protocol::packets::game::clientbound::ClientboundKeepAlive as GameRequest;
use mcrs_protocol::packets::game::serverbound::ServerboundKeepAlive as GameResponse;
use std::time::Instant;

pub struct KeepAlivePlugin;

impl Plugin for KeepAlivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(bevy_app::FixedPreUpdate, handle_keepalive);
        app.add_systems(bevy_app::FixedPreUpdate, new_connection);
        app.add_observer(handle_keepalive_response);
    }
}

#[derive(Component, Debug)]
pub struct KeepaliveState {
    pending: bool,
    time: Instant,
    challenge: i64,
}

pub fn new_connection(
    query: Query<(Entity, &ConnectionState), Changed<ConnectionState>>,
    mut commands: Commands,
) {
    for (entity, state) in query {
        if *state == ConnectionState::Login {
            commands.entity(entity).remove::<KeepaliveState>();
            continue;
        }

        commands.entity(entity).insert(KeepaliveState {
            pending: false,
            time: Instant::now(),
            challenge: 0,
        });
    }
}

pub fn handle_keepalive(
    mut query: Query<(
        Entity,
        &mut ServerSideConnection,
        &ConnectionState,
        &mut KeepaliveState,
    )>,
    mut commands: Commands,
) {
    let now = Instant::now();
    for (entity, mut con, conn_state, mut state) in query.iter_mut() {
        if *conn_state == ConnectionState::Login {
            continue;
        }

        if now.duration_since(state.time).as_secs() >= 15 {
            if state.pending {
                println!("Keepalive timed out, disconnecting");
                commands.entity(entity).remove::<ServerSideConnection>();
                continue;
            }

            state.challenge = rand::random();
            state.time = now;
            state.pending = true;
            println!("Keepalive sent: {:?}", state.challenge);
            let request = mcrs_protocol::packets::common::clientbound::KeepAlive {
                payload: state.challenge,
            };

            match conn_state {
                ConnectionState::Configuration => {
                    let pkt = ConfigurationRequest(request);
                    con.write_packet(&pkt);
                }
                ConnectionState::Game => {
                    let pkt = GameRequest(request);
                    con.write_packet(&pkt);
                }
                ConnectionState::Login => unreachable!(),
            }
        }
    }
}

pub fn handle_keepalive_response(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(&ServerSideConnection, &ConnectionState, &mut KeepaliveState)>,
    mut commands: Commands,
) {
    let Ok(((con, conn_state, mut state))) = query.get_mut(event.entity) else {
        return;
    };
    let keep_alive = match conn_state {
        ConnectionState::Configuration => {
            let (Some(pkt)) = event.decode::<ConfigurationResponse>() else {
                return;
            };
            pkt.0
        }
        ConnectionState::Game => {
            let (Some(pkt)) = event.decode::<GameResponse>() else {
                return;
            };
            pkt.0
        }
        _ => return,
    };
    println!("Keepalive response: {:?}", keep_alive);
    if !state.pending || keep_alive.payload != state.challenge {
        commands
            .entity(event.entity)
            .remove::<ServerSideConnection>();
        return;
    }

    state.pending = false;
}
