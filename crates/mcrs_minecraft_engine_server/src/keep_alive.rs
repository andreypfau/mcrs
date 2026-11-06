use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Query, Res};
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_time::{Real, Time};
use mcrs_minecraft_engine::muxer::latency::Latency;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{ConnectionState, Instant, ServerSideConnection};
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
        app.add_systems(
            bevy_app::FixedPreUpdate,
            (handle_keepalive, new_connection)
                .after(mcrs_minecraft_network::NetworkSet::SpawnConnections),
        );
        app.add_observer(on_configuration_response);
        app.add_observer(on_game_response);
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
                    ConnectionState::Handshaking
                    | ConnectionState::Status
                    | ConnectionState::Login => {
                        continue;
                    }
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

fn on_configuration_response(
    event: On<Received<ConfigurationResponse>>,
    query: Query<(&ServerSideConnection, &mut KeepAlive, Option<&Latency>)>,
    time: Res<Time<Real>>,
    commands: Commands,
) {
    if let Some(packet) = event.packet() {
        let arrived = arrival(event.arrival(), &time);
        handle_keepalive_response(event.entity, packet.0, query, arrived, commands);
    }
}

fn on_game_response(
    event: On<Received<GameResponse>>,
    query: Query<(&ServerSideConnection, &mut KeepAlive, Option<&Latency>)>,
    time: Res<Time<Real>>,
    commands: Commands,
) {
    if let Some(packet) = event.packet() {
        let arrived = arrival(event.arrival(), &time);
        handle_keepalive_response(event.entity, packet.0, query, arrived, commands);
    }
}

/// A response read in a later frame than it arrived still counts from its arrival, so the
/// latency does not grow by the wait for the frame.
fn arrival(arrival: Option<Instant>, time: &Time<Real>) -> Duration {
    arrival.map_or(time.elapsed(), |at| {
        at.saturating_duration_since(time.startup())
    })
}

fn handle_keepalive_response(
    entity: Entity,
    keep_alive: mcrs_minecraft_protocol::packets::common::serverbound::KeepAlive,
    mut query: Query<(&ServerSideConnection, &mut KeepAlive, Option<&Latency>)>,
    now: Duration,
    mut commands: Commands,
) {
    let Ok((con, mut state, latency)) = query.get_mut(entity) else {
        return;
    };
    debug!("Keepalive response: {:?}", keep_alive);
    match *state {
        KeepAlive::Awaiting { challenge, sent } if keep_alive.payload == challenge => {
            *state = KeepAlive::Idle { since: sent };
            let round_trip =
                i32::try_from(now.saturating_sub(sent).as_millis()).unwrap_or(i32::MAX);
            let sampled = latency.map_or(Latency(round_trip), |latency| latency.sample(round_trip));
            if latency != Some(&sampled) {
                commands.entity(entity).insert(sampled);
            }
        }
        _ => {
            warn!(
                "Keepalive failed for {}: got {} while {:?}",
                con.remote_addr(),
                keep_alive.payload,
                *state
            );
            commands.entity(entity).remove::<ServerSideConnection>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;
    use bytes::Bytes;
    use mcrs_minecraft_network::RawConnection;

    const CHALLENGE: i64 = 7;

    fn respond(world: &mut World, connection: Entity, arrival: Instant) {
        world.trigger(Received::<GameResponse>::arrived(
            connection,
            Bytes::copy_from_slice(&CHALLENGE.to_be_bytes()),
            arrival,
        ));
        world.flush();
    }

    #[test]
    fn latency_counts_to_the_arrival_and_is_not_rewritten_when_it_stays() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let startup = Instant::now();
        let mut world = World::new();
        let mut time = Time::<Real>::new(startup);
        time.update_with_instant(startup);
        time.update_with_instant(startup + Duration::from_millis(100));
        world.insert_resource(time);
        world.add_observer(on_game_response);
        let (raw, _outgoing, _inbound) = RawConnection::new_for_test_full(1);
        let awaiting = KeepAlive::Awaiting {
            challenge: CHALLENGE,
            sent: Duration::from_millis(40),
        };
        let connection = world
            .spawn((ServerSideConnection { raw: Box::new(raw) }, awaiting))
            .id();

        respond(&mut world, connection, startup + Duration::from_millis(60));
        assert_eq!(world.get::<Latency>(connection), Some(&Latency(20)));

        let changed = |world: &World| {
            world
                .entity(connection)
                .get_change_ticks::<Latency>()
                .map(|ticks| ticks.changed)
        };
        let measured = changed(&world);
        world.increment_change_tick();
        world.entity_mut(connection).insert(awaiting);
        respond(&mut world, connection, startup + Duration::from_millis(60));
        assert_eq!(
            changed(&world),
            measured,
            "a repeated round trip keeps the latency, so viewers get no update"
        );
    }
}
