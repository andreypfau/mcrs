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
    use mcrs_minecraft_protocol::Packet;
    use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
    use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundLoginAcknowledged;
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
            let ends_state = match state {
                // A connection in game is read by the server's bridge, which rate-limits it.
                Some(ConnectionState::Game) => return,
                Some(ConnectionState::Login) => Some(ServerboundLoginAcknowledged::ID),
                Some(ConnectionState::Configuration) => Some(ServerboundFinishConfiguration::ID),
                None => None,
            };
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
    use bevy_ecs::observer::On;
    use bevy_ecs::resource::Resource;
    use bevy_ecs::system::{Commands, IntoSystem, Query, ResMut, System};
    use bevy_ecs::world::World;
    use mcrs_minecraft_protocol::Encode;
    use mcrs_minecraft_protocol::packets::common::Brand;
    use mcrs_minecraft_protocol::packets::common::serverbound::{
        ClientInformation, KeepAlive, Payload,
    };
    use mcrs_minecraft_protocol::packets::configuration::serverbound::{
        ServerboundClientInformation, ServerboundCustomPayload, ServerboundFinishConfiguration,
    };
    use mcrs_minecraft_protocol::packets::game;
    use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundLoginAcknowledged;
    use mcrs_minecraft_protocol::packets::table::{configuration_serverbound, login_serverbound};
    use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
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

    fn packet<P: Encode + Packet>(packet: &P) -> ReceivedPacket {
        let mut body = Vec::new();
        packet.encode(&mut body).unwrap();
        ReceivedPacket {
            timestamp: Instant::now(),
            id: P::ID,
            payload: body.into(),
        }
    }

    fn run(world: &mut World) {
        let mut system = IntoSystem::into_system(loop_plugin::run_event_loop);
        system.initialize(world);
        let _ = system.run((), world);
        system.apply_deferred(world);
    }

    #[derive(Resource, Default)]
    struct Dispatched(Vec<(ConnectionState, i32)>);

    fn answer_like_the_server(
        event: On<ReceivedPacketEvent>,
        mut states: Query<&mut ConnectionState>,
        mut dispatched: ResMut<Dispatched>,
        mut commands: Commands,
    ) {
        let mut state = states.get_mut(event.entity).unwrap();
        dispatched.0.push((*state, event.id));
        match *state {
            ConnectionState::Login if event.decode::<ServerboundLoginAcknowledged>().is_some() => {
                commands
                    .entity(event.entity)
                    .insert(ConnectionState::Configuration);
            }
            ConnectionState::Configuration
                if event.decode::<ServerboundFinishConfiguration>().is_some() =>
            {
                *state = ConnectionState::Game;
            }
            _ => {}
        }
    }

    fn server_world() -> World {
        let mut world = World::new();
        world.init_resource::<PreGameDecodeCounts>();
        world.init_resource::<Dispatched>();
        world.add_observer(answer_like_the_server);
        world
    }

    fn counted_faults(world: &World) -> Vec<String> {
        let counts = world.resource::<PreGameDecodeCounts>();
        let mut faults = Vec::new();
        for (state, names) in [
            (ConnectionState::Login, login_serverbound::NAMES),
            (
                ConnectionState::Configuration,
                configuration_serverbound::NAMES,
            ),
        ] {
            let table = counts.counts(state).unwrap();
            for (row, name) in names.iter().enumerate() {
                if table.failures(row) > 0 {
                    faults.push(format!("{state:?} {name}: {}", table.failures(row)));
                }
            }
            if table.unknown() > 0 {
                faults.push(format!("{state:?} unknown id: {}", table.unknown()));
            }
        }
        faults
    }

    #[test]
    fn valid_frames_of_two_states_queued_together_count_nothing() {
        let mut world = server_world();
        let (_connection, tx) = spawn_connection(&mut world, ConnectionState::Login);
        tx.try_send(packet(&ServerboundLoginAcknowledged)).unwrap();
        tx.try_send(packet(&ServerboundCustomPayload::from(Payload::Brand(
            Brand { brand: "vanilla" },
        ))))
        .unwrap();
        tx.try_send(packet(&ServerboundClientInformation(ClientInformation {
            locale: "en_us",
            view_distance: 12,
            chat_mode: ChatMode::Enabled,
            chat_colors: true,
            displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
            main_arm: MainArm::Right,
            enable_text_filtering: false,
            allow_server_listings: true,
            particle_status: ParticleStatus::All,
        })))
        .unwrap();

        run(&mut world);
        run(&mut world);

        assert_eq!(
            world.resource::<Dispatched>().0,
            [
                (ConnectionState::Login, ServerboundLoginAcknowledged::ID),
                (ConnectionState::Configuration, ServerboundCustomPayload::ID),
                (
                    ConnectionState::Configuration,
                    ServerboundClientInformation::ID
                ),
            ]
        );
        assert_eq!(counted_faults(&world), Vec::<String>::new());
    }

    #[test]
    fn a_game_frame_behind_the_end_of_configuration_stays_queued() {
        let mut world = server_world();
        let (connection, tx) = spawn_connection(&mut world, ConnectionState::Configuration);
        let keep_alive = game::serverbound::ServerboundKeepAlive(KeepAlive { payload: 7 });
        tx.try_send(packet(&ServerboundFinishConfiguration))
            .unwrap();
        tx.try_send(packet(&keep_alive)).unwrap();

        run(&mut world);
        run(&mut world);

        assert_eq!(
            world.resource::<Dispatched>().0,
            [(
                ConnectionState::Configuration,
                ServerboundFinishConfiguration::ID
            )]
        );
        assert_eq!(counted_faults(&world), Vec::<String>::new());
        let mut connection = world.get_mut::<ServerSideConnection>(connection).unwrap();
        let queued = connection.raw.try_recv().unwrap().unwrap();
        assert_eq!(queued.id, game::serverbound::ServerboundKeepAlive::ID);
    }

    #[test]
    fn a_fault_behind_an_acknowledgement_nobody_accepts_is_counted_in_its_state() {
        let mut world = World::new();
        world.init_resource::<PreGameDecodeCounts>();
        let (_connection, tx) = spawn_connection(&mut world, ConnectionState::Login);
        tx.try_send(packet(&ServerboundLoginAcknowledged)).unwrap();
        tx.try_send(frame(login_serverbound::NAMES.len() as i32))
            .unwrap();

        run(&mut world);
        run(&mut world);

        assert_eq!(counted_faults(&world), ["Login unknown id: 1"]);
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
