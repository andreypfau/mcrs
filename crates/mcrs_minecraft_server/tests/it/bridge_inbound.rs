use crate::mock_connection;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageRegistry;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{IntoSystem, RunSystemOnce, System};
use bevy_ecs::world::World;
use bytes::BytesMut;
use mcrs_minecraft_engine::bus::{InboundPlayerPacket, OutboundPlayerPacket};
use mcrs_minecraft_engine::muxer::bridge::bridge_inbound;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSessionCounter, Session, SessionPlacement,
};
use mcrs_minecraft_nbt::NbtBudget;
use mcrs_minecraft_network::event::{Received, run_event_loop};
use mcrs_minecraft_network::inbound_rate::{InboundPacketRate, InboundRateLimit};
use mcrs_minecraft_network::metrics::{BridgeTelemetry, PacketDispatcher};
use mcrs_minecraft_network::{
    ConnectionState, NetworkPlugin, ReceivedPacket, ServerSideConnection,
};
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::decode::PacketDecoder;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundHello;
use mcrs_minecraft_protocol::packets::table::{
    Row, configuration_serverbound, login_serverbound, row_of,
};
use mcrs_minecraft_protocol::packets::{configuration, game};
use mcrs_minecraft_protocol::{Encode, VarInt};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundEditBook, ServerboundSetCreativeModeSlot,
};
use mcrs_minecraft_protocol_game::table::game_serverbound;

use std::net::{Ipv4Addr, SocketAddrV4};

use std::time::{Duration, Instant};
use tokio::sync::mpsc;

fn build_inbound_world() -> World {
    let mut world = World::new();
    MessageRegistry::register_message::<OutboundPlayerPacket>(&mut world);
    MessageRegistry::register_message::<InboundPlayerPacket>(&mut world);
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<DimInputs>();
    world.init_resource::<BridgeTelemetry>();
    world.insert_resource(
        PacketDispatcher::serverbound()
            .with_play(mcrs_minecraft_engine::muxer::game_packets::GAME_SERVERBOUND),
    );
    world
}

fn spawn_ingame_connection(world: &mut World) -> (Entity, mpsc::Sender<ReceivedPacket>) {
    let (raw, _outgoing_rx, inbound_tx) = mock_connection::make_mock_raw_connection_full();
    let entity = world
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Game,
            InboundPacketRate::default(),
            OutboundQueue::default(),
        ))
        .id();
    (entity, inbound_tx)
}

fn register_player(world: &mut World, dim: Entity, in_dim_entity: Option<Entity>) -> Entity {
    let session = world.resource_mut::<PlayerSessionCounter>().next();
    let place = match in_dim_entity {
        Some(_) => Place::InDim(dim),
        None => Place::Joining(dim),
    };
    world
        .spawn(SessionBundle::placed(
            session,
            SessionPlacement::new(place, 0),
        ))
        .id()
}

fn attach_anchor(world: &mut World, socket: Entity, player: Entity) {
    world.entity_mut(socket).insert(HostAnchorRef(player));
}

fn make_received_packet(seq: u32) -> ReceivedPacket {
    frame(keep_alive_row(), &i64::from(seq).to_be_bytes())
}

fn run_inbound(world: &mut World) {
    let mut sys = IntoSystem::into_system(bridge_inbound);
    sys.initialize(world);
    let _ = sys.run((), world);
    sys.apply_deferred(world);
}

#[derive(Resource, Default)]
struct EventCounter {
    count: usize,
}

#[test]
fn bridge_inbound_emits_received_packet_event() {
    let mut world = build_inbound_world();
    world.init_resource::<EventCounter>();

    world.add_observer(
        |_ev: On<Received<game::serverbound::ServerboundKeepAlive>>,
         mut counter: bevy_ecs::system::ResMut<EventCounter>| {
            counter.count += 1;
        },
    );

    let dim_a = Entity::from_raw_u32(10).expect("nonzero");
    let dim_b = Entity::from_raw_u32(11).expect("nonzero");
    let in_dim_a = Entity::from_raw_u32(30).expect("nonzero");
    let in_dim_b = Entity::from_raw_u32(31).expect("nonzero");

    let (_socket_a, tx_a) = spawn_ingame_connection(&mut world);
    let (_socket_b, tx_b) = spawn_ingame_connection(&mut world);

    register_player(&mut world, dim_a, Some(in_dim_a));
    register_player(&mut world, dim_b, Some(in_dim_b));

    tx_a.try_send(make_received_packet(1)).unwrap();
    tx_b.try_send(make_received_packet(2)).unwrap();

    run_inbound(&mut world);

    let counter = world.resource::<EventCounter>();
    assert_eq!(
        counter.count, 2,
        "bridge_inbound must emit one typed event per drained packet"
    );
}

fn connected_player(world: &mut World) -> (Entity, mpsc::Sender<ReceivedPacket>) {
    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let in_dim = Entity::from_raw_u32(30).expect("nonzero");
    let (socket, tx) = spawn_ingame_connection(world);
    let player = register_player(world, dim, Some(in_dim));
    attach_anchor(world, socket, player);
    (socket, tx)
}

fn arrive(world: &mut World, tx: &mpsc::Sender<ReceivedPacket>, arrival: Instant, count: u32) {
    for seq in 0..count {
        let mut packet = make_received_packet(seq);
        packet.timestamp = arrival;
        tx.try_send(packet).unwrap();
    }
    run_inbound(world);
}

#[test]
fn a_burst_of_100_packets_after_a_stall_of_3_seconds_keeps_the_connection() {
    for limit in [InboundRateLimit::default(), InboundRateLimit(50)] {
        let mut world = build_inbound_world();
        world.insert_resource(limit);
        let (socket, tx) = connected_player(&mut world);
        let start = Instant::now();

        arrive(&mut world, &tx, start, 20);
        arrive(&mut world, &tx, start + Duration::from_secs(3), 100);
        arrive(&mut world, &tx, start + Duration::from_secs(4), 1);

        assert!(
            world.get::<ServerSideConnection>(socket).is_some(),
            "MVC-INV-008: {limit:?}"
        );
        assert_eq!(world.resource::<BridgeTelemetry>().kick_flood_total, 0);
    }
}

#[test]
fn a_rate_above_the_limit_ends_the_connection() {
    let mut world = build_inbound_world();
    world.insert_resource(InboundRateLimit(50));
    let (socket, tx) = connected_player(&mut world);
    let start = Instant::now();

    for second in 0..3 {
        arrive(&mut world, &tx, start + Duration::from_secs(second), 100);
    }
    assert!(world.get::<ServerSideConnection>(socket).is_some());
    arrive(&mut world, &tx, start + Duration::from_secs(3), 1);

    assert!(
        world.get::<ServerSideConnection>(socket).is_none(),
        "MVC-VAN-019: the average of the third second is 57.8"
    );
    assert_eq!(world.resource::<BridgeTelemetry>().kick_flood_total, 1);
}

#[test]
fn a_new_connection_has_an_outbound_queue_at_once() {
    use mcrs_minecraft_engine::muxer::bridge::attach_outbound_queue;

    let mut world = World::new();
    world.add_observer(attach_outbound_queue);
    let (raw, _rx) = mock_connection::make_mock_raw_connection();
    let socket = world
        .spawn(ServerSideConnection { raw: Box::new(raw) })
        .id();

    assert!(world.get::<OutboundQueue>(socket).is_some());
}

#[test]
fn disconnect_clears_pending() {
    use mcrs_minecraft_engine::muxer::disconnect::{LeavingSessions, process_disconnect};

    let mut world = World::new();
    world.init_resource::<PlayerSessionCounter>();
    world.init_resource::<DimInputs>();

    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let in_dim = Entity::from_raw_u32(30).expect("nonzero");

    let (socket, _tx) = spawn_ingame_connection(&mut world);
    let player = register_player(&mut world, dim, Some(in_dim));
    attach_anchor(&mut world, socket, player);

    assert!(
        world.get::<Session>(player).is_some(),
        "session should exist before disconnect"
    );

    assert!(
        world.get::<OutboundQueue>(socket).is_some(),
        "OutboundQueue should exist before disconnect"
    );

    world
        .run_system_once(
            move |mut commands: bevy_ecs::prelude::Commands,
                  mut sessions: LeavingSessions,
                  mut dim_channels: bevy_ecs::system::ResMut<DimInputs>| {
                process_disconnect(
                    player,
                    sessions.reborrow(),
                    &mut dim_channels,
                    commands.reborrow(),
                );
            },
        )
        .expect("process_disconnect system ran");

    assert!(
        world.get_entity(player).is_err(),
        "the session must be gone after disconnect"
    );

    assert!(
        world.get::<OutboundQueue>(socket).is_none(),
        "OutboundQueue must be removed from socket entity after disconnect"
    );
}

fn frame(id: i32, payload: &[u8]) -> ReceivedPacket {
    ReceivedPacket {
        timestamp: Instant::now(),
        id,
        payload: bytes::Bytes::copy_from_slice(payload),
        verdict: None,
    }
}

fn keep_alive_row() -> i32 {
    row_of(game_serverbound::NAMES, "keep_alive").unwrap()
}

fn row_without_structure() -> i32 {
    game_serverbound::NAMES
        .iter()
        .position(|name| !game_serverbound::TYPED.contains(name))
        .expect("a serverbound game row without a structure") as i32
}

fn game_counts(world: &World) -> &mcrs_minecraft_network::metrics::TableCounts {
    world
        .resource::<PacketDispatcher>()
        .counts(ConnectionState::Game)
        .unwrap()
}

#[derive(Resource, Default)]
struct DeliveredIds(Vec<i32>);

fn deliver<R: Row>(id: i32) -> impl Fn(On<Received<R>>, bevy_ecs::system::ResMut<DeliveredIds>) {
    move |_, mut delivered| delivered.0.push(id)
}

#[test]
fn a_failed_decode_is_counted_under_its_packet() {
    let mut world = build_inbound_world();
    let (_short, short_tx) = spawn_ingame_connection(&mut world);
    let (_long, long_tx) = spawn_ingame_connection(&mut world);
    short_tx
        .try_send(frame(keep_alive_row(), &[0, 0, 0]))
        .unwrap();
    long_tx.try_send(frame(keep_alive_row(), &[0; 9])).unwrap();

    run_inbound(&mut world);

    let counts = game_counts(&world);
    assert_eq!(counts.failures(keep_alive_row() as usize), 2);
    assert_eq!(counts.failures_total(), 2);
    assert_eq!(counts.unknown(), 0);
}

#[test]
fn a_row_without_a_structure_counts_nothing() {
    let mut world = build_inbound_world();
    let (socket, tx) = spawn_ingame_connection(&mut world);
    tx.try_send(frame(row_without_structure(), &[5; 12]))
        .unwrap();
    tx.try_send(frame(keep_alive_row(), &7i64.to_be_bytes()))
        .unwrap();

    run_inbound(&mut world);

    assert_eq!(game_counts(&world).failures_total(), 0);
    assert_eq!(game_counts(&world).unknown(), 0);
    assert!(world.entity(socket).contains::<ServerSideConnection>());
}

#[test]
fn each_loop_counts_only_its_own_connections() {
    let mut world = build_inbound_world();
    let (_game, game_tx) = spawn_ingame_connection(&mut world);
    let (raw, _outgoing_rx, login_tx) = mock_connection::make_mock_raw_connection_full();
    world.spawn((
        ServerSideConnection { raw: Box::new(raw) },
        ConnectionState::Login,
    ));
    let unknown_everywhere = 10_000;
    game_tx.try_send(frame(unknown_everywhere, &[])).unwrap();
    login_tx.try_send(frame(unknown_everywhere, &[])).unwrap();

    run_inbound(&mut world);
    assert_eq!(game_counts(&world).unknown(), 1);
    let login = |world: &World| {
        world
            .resource::<PacketDispatcher>()
            .counts(ConnectionState::Login)
            .unwrap()
            .unknown()
    };
    assert_eq!(login(&world), 0);

    let mut sys = IntoSystem::into_system(run_event_loop);
    sys.initialize(&mut world);
    let _ = sys.run((), &mut world);
    sys.apply_deferred(&mut world);
    assert_eq!(login(&world), 1);
    assert_eq!(game_counts(&world).unknown(), 1);
    let configuration = world
        .resource::<PacketDispatcher>()
        .counts(ConnectionState::Configuration)
        .unwrap()
        .unknown();
    assert_eq!(configuration, 0);
}

fn disconnect_reason(state: ConnectionState, body: &[u8]) -> serde_json::Value {
    let body = &mut &body[..];
    match state {
        ConnectionState::Login => {
            serde_json::from_str(ClientboundLoginDisconnect::decode(body).unwrap().reason.0)
                .unwrap()
        }
        ConnectionState::Configuration => serde_json::to_value(
            configuration::clientbound::ClientboundDisconnect::decode(body)
                .unwrap()
                .reason,
        )
        .unwrap(),
        ConnectionState::Game => serde_json::to_value(
            game::clientbound::ClientboundDisconnect::decode(body)
                .unwrap()
                .reason,
        )
        .unwrap(),
        ConnectionState::Handshaking | ConnectionState::Status => unreachable!(),
    }
}

#[test]
fn a_malformed_frame_disconnects_its_client_in_the_protocol_of_its_state() {
    let hello = row_of(login_serverbound::NAMES, "hello").unwrap();
    let accept = row_of(game_serverbound::NAMES, "accept_teleportation").unwrap();
    let unknown = configuration_serverbound::NAMES.len() as i32;
    let decoder = "io.netty.handler.codec.DecoderException";
    for (state, sent, disconnect, cause) in [
        (
            ConnectionState::Login,
            frame(hello, &[]),
            ClientboundLoginDisconnect::ID,
            format!("{decoder}: Failed to decode packet 'serverbound/minecraft:hello'"),
        ),
        (
            ConnectionState::Configuration,
            frame(unknown, &[]),
            configuration::clientbound::ClientboundDisconnect::ID,
            format!("{decoder}: Received unknown packet id {unknown}"),
        ),
        (
            ConnectionState::Game,
            frame(-1, &[]),
            game::clientbound::ClientboundDisconnect::ID,
            format!("{decoder}: Received unknown packet id -1"),
        ),
        (
            ConnectionState::Game,
            frame(accept, &[1]),
            game::clientbound::ClientboundDisconnect::ID,
            format!(
                "{decoder}: Failed to decode packet 'serverbound/minecraft:accept_teleportation'"
            ),
        ),
        (
            ConnectionState::Game,
            frame(keep_alive_row(), &[0; 9]),
            game::clientbound::ClientboundDisconnect::ID,
            format!(
                "{decoder}: java.io.IOException: Packet play/serverbound/minecraft:keep_alive was larger than I expected, found 1 bytes extra whilst reading packet serverbound/minecraft:keep_alive"
            ),
        ),
    ] {
        let mut world = build_inbound_world();
        world.init_resource::<DeliveredIds>();
        world.add_observer(deliver::<game::serverbound::ServerboundKeepAlive>(
            keep_alive_row(),
        ));
        world.add_observer(deliver::<ServerboundHello<'static>>(hello));
        let (raw, mut outgoing, tx) = mock_connection::make_mock_raw_connection_full();
        let socket = world
            .spawn((
                ServerSideConnection { raw: Box::new(raw) },
                state,
                InboundPacketRate::default(),
                OutboundQueue::default(),
            ))
            .id();
        tx.try_send(sent).unwrap();

        run_inbound(&mut world);
        let mut pre_game = IntoSystem::into_system(run_event_loop);
        pre_game.initialize(&mut world);
        let _ = pre_game.run((), &mut world);
        pre_game.apply_deferred(&mut world);

        assert_eq!(world.resource::<DeliveredIds>().0, [0; 0], "{state:?}");
        assert!(
            !world.entity(socket).contains::<ServerSideConnection>(),
            "{state:?} is still connected"
        );
        let mut frames = PacketDecoder::new();
        while let Ok(blob) = outgoing.try_recv() {
            frames.queue_bytes(BytesMut::from(&blob[..]));
        }
        let received: Vec<_> = std::iter::from_fn(|| frames.try_next_packet().unwrap()).collect();
        let [received] = received.as_slice() else {
            panic!("{state:?}: {received:?}");
        };
        assert_eq!(received.id, disconnect, "{state:?}");
        assert_eq!(
            disconnect_reason(state, &received.body),
            serde_json::json!({
                "translate": "disconnect.genericReason",
                "with": [format!("Internal Exception: {cause}")],
            }),
            "{state:?}"
        );
    }
}

#[test]
fn frames_after_configuration_ack_use_the_configuration_table() {
    let mut world = build_inbound_world();
    world.init_resource::<DeliveredIds>();
    world.add_observer(
        |event: On<Received<game::serverbound::ServerboundConfigurationAcknowledged>>,
         mut states: bevy_ecs::system::Query<&mut ConnectionState>| {
            *states.get_mut(event.entity).unwrap() = ConnectionState::Configuration;
        },
    );
    world.add_observer(
        |event: On<Received<configuration::serverbound::ServerboundKeepAlive>>,
         mut delivered: bevy_ecs::system::ResMut<DeliveredIds>| {
            delivered.0.push(event.packet().unwrap().0.payload as i32);
        },
    );
    let (socket, tx) = spawn_ingame_connection(&mut world);
    let acknowledged = game::serverbound::ServerboundConfigurationAcknowledged::ID;
    let keep_alive = configuration::serverbound::ServerboundKeepAlive::ID;
    tx.try_send(frame(acknowledged, &[])).unwrap();
    tx.try_send(frame(keep_alive, &7i64.to_be_bytes())).unwrap();

    run_inbound(&mut world);
    let mut pre_game = IntoSystem::into_system(run_event_loop);
    pre_game.initialize(&mut world);
    let _ = pre_game.run((), &mut world);
    pre_game.apply_deferred(&mut world);

    assert!(world.entity(socket).contains::<ServerSideConnection>());
    assert_eq!(world.resource::<DeliveredIds>().0, [7]);
    assert_eq!(game_counts(&world).failures_total(), 0);
}

fn var_int(value: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    VarInt(i32::try_from(value).unwrap())
        .encode(&mut bytes)
        .unwrap();
    bytes
}

fn creative_slot_with_custom_data(length: usize) -> Vec<u8> {
    let mut nbt = vec![10, 7, 0, 1, b'a'];
    nbt.extend_from_slice(&i32::try_from(length).unwrap().to_be_bytes());
    nbt.resize(nbt.len() + length, 0);
    nbt.push(0);
    [
        36i16.to_be_bytes().to_vec(),
        var_int(1),
        var_int(1),
        var_int(1),
        var_int(0),
        var_int(0),
        var_int(nbt.len()),
        nbt,
    ]
    .concat()
}

fn edit_book_of(pages: usize) -> Vec<u8> {
    let mut bytes = [var_int(0), var_int(pages)].concat();
    for _ in 0..pages {
        bytes.extend_from_slice(&[1, b'x']);
    }
    bytes.push(0);
    bytes
}

fn plugin_dispatcher(bytes: u64) -> PacketDispatcher {
    let mut app = App::new();
    app.add_plugins(NetworkPlugin {
        address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
        announce_on_lan: false,
        nbt_budget: NbtBudget {
            bytes,
            tags: 262_144,
        },
        ..Default::default()
    });
    let mut dispatcher = app
        .world_mut()
        .remove_resource::<PacketDispatcher>()
        .expect("the network plugin inserts the dispatcher");
    dispatcher.set_play(mcrs_minecraft_engine::muxer::game_packets::GAME_SERVERBOUND);
    dispatcher
}

#[test]
fn a_packet_past_the_nbt_budget_of_the_plugin_disconnects_and_is_not_delivered() {
    let creative = row_of(game_serverbound::NAMES, "set_creative_mode_slot").unwrap();
    let edit_book = row_of(game_serverbound::NAMES, "edit_book").unwrap();
    let decoder = "io.netty.handler.codec.DecoderException";
    for (budget, id, body, delivered) in [
        (
            33_554_432,
            creative,
            creative_slot_with_custom_data(2_097_148),
            true,
        ),
        (
            1_000_000,
            creative,
            creative_slot_with_custom_data(2_097_148),
            false,
        ),
        (33_554_432, edit_book, edit_book_of(101), false),
    ] {
        let mut world = build_inbound_world();
        world.insert_resource(plugin_dispatcher(budget));
        world.init_resource::<DeliveredIds>();
        world.add_observer(deliver::<ServerboundSetCreativeModeSlot>(creative));
        world.add_observer(deliver::<ServerboundEditBook<'static>>(edit_book));
        let (raw, mut outgoing, tx) = mock_connection::make_mock_raw_connection_full();
        let socket = world
            .spawn((
                ServerSideConnection { raw: Box::new(raw) },
                ConnectionState::Game,
                InboundPacketRate::default(),
                OutboundQueue::default(),
            ))
            .id();
        tx.try_send(frame(id, &body)).unwrap();

        run_inbound(&mut world);

        let name = game_serverbound::NAMES[id as usize];
        let connected = world.entity(socket).contains::<ServerSideConnection>();
        if delivered {
            assert_eq!(world.resource::<DeliveredIds>().0, [id], "{name} {budget}");
            assert!(connected, "{name} {budget} keeps the connection");
            continue;
        }
        assert_eq!(
            world.resource::<DeliveredIds>().0,
            [0; 0],
            "{name} {budget}"
        );
        assert!(!connected, "{name} {budget} is still connected");
        let mut frames = PacketDecoder::new();
        while let Ok(blob) = outgoing.try_recv() {
            frames.queue_bytes(BytesMut::from(&blob[..]));
        }
        let received: Vec<_> = std::iter::from_fn(|| frames.try_next_packet().unwrap()).collect();
        let [received] = received.as_slice() else {
            panic!("{name} {budget}: {received:?}");
        };
        assert_eq!(
            disconnect_reason(ConnectionState::Game, &received.body),
            serde_json::json!({
                "translate": "disconnect.genericReason",
                "with": [format!(
                    "Internal Exception: {decoder}: Failed to decode packet 'serverbound/minecraft:{name}'"
                )],
            }),
            "{name} {budget}"
        );
    }
}

#[test]
fn a_packet_carries_the_tick_acknowledged_at_its_arrival_and_only_our_pongs_are_taken() {
    use mcrs_minecraft_engine::muxer::probes::Probes;
    use mcrs_minecraft_engine::muxer::session::PendingInbound;

    let mut world = build_inbound_world();
    let dim = Entity::from_raw_u32(10).expect("nonzero");
    let (socket, tx) = spawn_ingame_connection(&mut world);
    let player = register_player(&mut world, dim, None);
    attach_anchor(&mut world, socket, player);
    let mut probes = Probes::default();
    for _ in 0..3 {
        probes.pings.ping(Duration::ZERO);
        probes.pings.end_tick(Duration::ZERO);
    }
    world.entity_mut(socket).insert(probes);

    let pong_row = row_of(game_serverbound::NAMES, "pong").unwrap();
    let pong = |id: i32| {
        let mut packet = frame(pong_row, &id.to_be_bytes());
        packet.verdict = Some(mcrs_minecraft_move_validate::check_serverbound(
            pong_row,
            &id.to_be_bytes(),
        ));
        packet
    };
    for packet in [
        make_received_packet(0),
        pong(1),
        make_received_packet(1),
        pong(7),
        pong(2),
        make_received_packet(2),
    ] {
        tx.try_send(packet).unwrap();
    }
    run_inbound(&mut world);

    let delivered: Vec<(i32, u64)> = world
        .get::<PendingInbound>(player)
        .unwrap()
        .0
        .iter()
        .map(|packet| (packet.id, packet.acknowledged_tick))
        .collect();
    let keep_alive = keep_alive_row();
    assert_eq!(
        delivered,
        [
            (keep_alive, 0),
            (keep_alive, 1),
            (pong_row, 1),
            (keep_alive, 2)
        ]
    );
}
