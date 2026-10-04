use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, IntoSystem, Query, ResMut, System};
use bevy_ecs::world::World;
use bytes::Bytes;
use mcrs_minecraft_network::event::{
    PRE_GAME_FRAMES_PER_PASS, ReceivedPacketEvent, run_event_loop,
};
use mcrs_minecraft_network::inbound_rate::{INBOUND_BUCKET_CAP, INBOUND_KICK_OVERFLOW_PACKETS};
use mcrs_minecraft_network::metrics::{BridgeTelemetry, PreGameDecodeCounts};
use mcrs_minecraft_network::{
    ConnectionState, Instant, RawConnection, ReceivedPacket, ServerSideConnection,
};
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::serverbound::{
    ClientInformation, KeepAlive, Payload,
};
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundCustomPayload, ServerboundFinishConfiguration,
    ServerboundKeepAlive, ServerboundSelectKnownPacks,
};
use mcrs_minecraft_protocol::packets::game;
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::packets::table::{configuration_serverbound, login_serverbound};
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::{Encode, Packet};
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

fn frame_at<P: Encode + Packet>(packet: &P, timestamp: Instant) -> ReceivedPacket {
    let mut body = Vec::new();
    packet.encode(&mut body).unwrap();
    ReceivedPacket {
        timestamp,
        id: P::ID,
        payload: body.into(),
    }
}

fn frame<P: Encode + Packet>(packet: &P) -> ReceivedPacket {
    frame_at(packet, Instant::now())
}

fn keep_alive(payload: i64, timestamp: Instant) -> ReceivedPacket {
    frame_at(&ServerboundKeepAlive(KeepAlive { payload }), timestamp)
}

fn run(world: &mut World) {
    let mut system = IntoSystem::into_system(run_event_loop);
    system.initialize(world);
    let _ = system.run((), world);
    system.apply_deferred(world);
}

fn server_world() -> World {
    let mut world = World::new();
    world.init_resource::<PreGameDecodeCounts>();
    world.init_resource::<BridgeTelemetry>();
    world
}

fn raw_frame(id: i32) -> ReceivedPacket {
    ReceivedPacket {
        timestamp: Instant::now(),
        id,
        payload: Bytes::new(),
    }
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

#[derive(Resource, Default)]
struct KeepAlives(Vec<i64>);

fn record_keep_alives(event: On<ReceivedPacketEvent>, mut seen: ResMut<KeepAlives>) {
    if let Some(packet) = event.decode::<ServerboundKeepAlive>() {
        seen.0.push(packet.0.payload);
    }
}

#[test]
fn a_pass_reads_its_budget_and_leaves_the_rest_queued_in_order() {
    let mut world = server_world();
    world.init_resource::<KeepAlives>();
    world.add_observer(record_keep_alives);
    let (connection, tx) = spawn_connection(&mut world, ConnectionState::Configuration);
    let queued = 3 * PRE_GAME_FRAMES_PER_PASS as i64;
    assert!(queued < i64::from(INBOUND_BUCKET_CAP));
    let now = Instant::now();
    for payload in 0..queued {
        tx.try_send(keep_alive(payload, now)).unwrap();
    }

    for pass in 1..=3 {
        run(&mut world);
        let read = pass * PRE_GAME_FRAMES_PER_PASS as i64;
        assert_eq!(
            world.resource::<KeepAlives>().0,
            (0..read).collect::<Vec<_>>(),
            "pass {pass}"
        );
    }

    assert!(world.entity(connection).contains::<ServerSideConnection>());
    assert_eq!(world.resource::<BridgeTelemetry>().kick_flood_total, 0);
    assert_eq!(counted_faults(&world), Vec::<String>::new());
}

#[test]
fn a_flood_before_the_game_state_is_cut_off_like_one_in_it() {
    let mut world = server_world();
    world.init_resource::<KeepAlives>();
    world.add_observer(record_keep_alives);
    let (connection, tx) = spawn_connection(&mut world, ConnectionState::Configuration);
    let allowed = i64::from(INBOUND_BUCKET_CAP) + i64::from(INBOUND_KICK_OVERFLOW_PACKETS) - 1;
    let queued = allowed + 10;
    let now = Instant::now();
    for payload in 0..queued {
        tx.try_send(keep_alive(payload, now)).unwrap();
    }

    for _ in 0..queued {
        run(&mut world);
    }

    assert!(!world.entity(connection).contains::<ServerSideConnection>());
    assert_eq!(world.resource::<BridgeTelemetry>().kick_flood_total, 1);
    assert_eq!(
        world.resource::<KeepAlives>().0,
        (0..allowed).collect::<Vec<_>>()
    );
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

#[test]
fn a_login_and_configuration_sent_in_one_burst_reaches_the_game_state() {
    let mut world = server_world();
    world.init_resource::<Dispatched>();
    world.add_observer(answer_like_the_server);
    let (connection, tx) = spawn_connection(&mut world, ConnectionState::Login);

    let mut hello = vec![5];
    hello.extend_from_slice(b"Steve");
    hello.extend_from_slice(&[0; 16]);
    tx.try_send(ReceivedPacket {
        timestamp: Instant::now(),
        id: ServerboundHello::ID,
        payload: Bytes::from(hello),
    })
    .unwrap();
    tx.try_send(frame(&ServerboundLoginAcknowledged)).unwrap();
    tx.try_send(frame(&ServerboundCustomPayload::from(Payload::Brand(
        Brand { brand: "vanilla" },
    ))))
    .unwrap();
    tx.try_send(frame(&ServerboundClientInformation(ClientInformation {
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
    tx.try_send(frame(&ServerboundSelectKnownPacks {
        known_packs: vec![KnownPack {
            namespace: "minecraft",
            id: "core",
            version: "1",
        }],
    }))
    .unwrap();
    tx.try_send(frame(&ServerboundFinishConfiguration)).unwrap();
    let first_game_frame = game::serverbound::ServerboundKeepAlive(KeepAlive { payload: 7 });
    tx.try_send(frame(&first_game_frame)).unwrap();

    run(&mut world);
    run(&mut world);
    run(&mut world);

    assert_eq!(
        world.resource::<Dispatched>().0,
        [
            (ConnectionState::Login, ServerboundHello::ID),
            (ConnectionState::Login, ServerboundLoginAcknowledged::ID),
            (ConnectionState::Configuration, ServerboundCustomPayload::ID),
            (
                ConnectionState::Configuration,
                ServerboundClientInformation::ID
            ),
            (
                ConnectionState::Configuration,
                ServerboundSelectKnownPacks::ID
            ),
            (
                ConnectionState::Configuration,
                ServerboundFinishConfiguration::ID
            ),
        ]
    );
    assert_eq!(
        world.get::<ConnectionState>(connection),
        Some(&ConnectionState::Game)
    );
    assert_eq!(world.resource::<BridgeTelemetry>().kick_flood_total, 0);
    assert_eq!(counted_faults(&world), Vec::<String>::new());
    let mut connection = world.get_mut::<ServerSideConnection>(connection).unwrap();
    let queued = connection.raw.try_recv().unwrap().unwrap();
    assert_eq!(queued.id, game::serverbound::ServerboundKeepAlive::ID);
}

#[test]
fn a_fault_behind_an_acknowledgement_nobody_accepts_is_counted_in_its_state() {
    let mut world = server_world();
    let (_connection, tx) = spawn_connection(&mut world, ConnectionState::Login);
    tx.try_send(frame(&ServerboundLoginAcknowledged)).unwrap();
    tx.try_send(raw_frame(login_serverbound::NAMES.len() as i32))
        .unwrap();

    run(&mut world);
    run(&mut world);

    assert_eq!(counted_faults(&world), ["Login unknown id: 1"]);
}

#[test]
fn an_unknown_id_before_the_game_state_is_counted_and_nothing_is_despawned() {
    let mut world = server_world();
    let (login, login_tx) = spawn_connection(&mut world, ConnectionState::Login);
    let (configuration, configuration_tx) =
        spawn_connection(&mut world, ConnectionState::Configuration);
    login_tx
        .try_send(raw_frame(login_serverbound::NAMES.len() as i32))
        .unwrap();
    configuration_tx
        .try_send(raw_frame(configuration_serverbound::NAMES.len() as i32 + 3))
        .unwrap();
    configuration_tx.try_send(raw_frame(-1)).unwrap();

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
