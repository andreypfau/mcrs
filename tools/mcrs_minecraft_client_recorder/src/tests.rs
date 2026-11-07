use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use bytes::BytesMut;
use mcrs_minecraft_client_ticks::Abilities;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Move;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::CompressionThreshold;
use mcrs_minecraft_protocol::Encode;
use mcrs_minecraft_protocol::PacketDecoder;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::frame::encode_frame;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::table::configuration_clientbound;
use mcrs_minecraft_protocol::packets::table::configuration_serverbound;
use mcrs_minecraft_protocol::packets::table::game_clientbound;
use mcrs_minecraft_protocol::packets::table::game_serverbound;
use mcrs_minecraft_protocol::packets::table::login_clientbound;
use mcrs_minecraft_protocol::packets::table::login_serverbound;
use mcrs_minecraft_protocol::packets::table::row_of;

use crate::delay::Delay;
use crate::delay::parse_trace;
use crate::names::Names;
use crate::proxy::Connection;
use crate::proxy::Observed;
use crate::scenario::Scenario;

/// One end of a socket pair, as a peer of the proxy sees it.
struct Peer {
    stream: TcpStream,
    decoder: PacketDecoder,
    threshold: i32,
}

impl Peer {
    fn send(&mut self, table: &[&str], name: &str, body: &[u8]) {
        let mut packet = Vec::new();
        VarInt(row_of(table, name).unwrap())
            .encode(&mut packet)
            .unwrap();
        packet.extend_from_slice(body);
        let mut frame = BytesMut::from(&packet[..]);
        encode_frame(
            &mut frame,
            0,
            CompressionThreshold(self.threshold),
            &mut None,
        )
        .unwrap();
        self.stream.write_all(&frame).unwrap();
    }

    fn compress(&mut self) {
        self.threshold = 0;
        self.decoder.set_compression(CompressionThreshold(0));
    }

    /// The packets that reached this peer, as `(id, body)`.
    fn received(&mut self) -> Vec<(i32, Vec<u8>)> {
        let mut chunk = [0; 4096];
        while let Ok(read) = self.stream.read(&mut chunk) {
            self.decoder.queue_bytes(BytesMut::from(&chunk[..read]));
        }
        let mut packets = Vec::new();
        while let Some(frame) = self.decoder.try_next_packet().unwrap() {
            packets.push((frame.id, frame.body.to_vec()));
        }
        packets
    }
}

fn pair() -> (Peer, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let peer = Peer {
        stream,
        decoder: PacketDecoder::new(),
        threshold: -1,
    };
    (peer, listener.accept().unwrap().0)
}

fn id(table: &[&str], name: &str) -> i32 {
    row_of(table, name).unwrap()
}

fn pump(connection: &mut Connection, names: &Names, observed: &mut Vec<Observed>) {
    for _ in 0..5 {
        assert!(
            connection.poll(names, observed).unwrap(),
            "both peers stay open"
        );
    }
}

/// Takes both peers through the handshake, a compressed login and the configuration.
fn join(client: &mut Peer, server: &mut Peer, connection: &mut Connection, names: &Names) {
    let advance = |connection: &mut Connection| pump(connection, names, &mut Vec::new());
    let mut handshake = Vec::new();
    ServerboundHandshake {
        protocol_version: VarInt(0),
        server_address: Bounded("localhost"),
        server_port: 25565,
        intent: Intent::Login,
    }
    .encode(&mut handshake)
    .unwrap();
    client.send(&["intention"], "intention", &handshake);
    advance(connection);
    server.send(login_clientbound::NAMES, "setup_compression", &[0]);
    advance(connection);
    assert_eq!(
        client.received().len(),
        1,
        "the client has the compression packet"
    );
    server.compress();
    client.compress();
    server.send(login_clientbound::NAMES, "login_finished", &[1, 2, 3]);
    client.send(login_serverbound::NAMES, "login_acknowledged", &[]);
    advance(connection);
    server.send(
        configuration_clientbound::NAMES,
        "finish_configuration",
        &[],
    );
    client.send(
        configuration_serverbound::NAMES,
        "finish_configuration",
        &[],
    );
    advance(connection);
}

#[test]
fn a_clientbound_change_applies_at_the_answer_to_the_probe_after_it() {
    let names = Names::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let (mut client, proxy_client) = pair();
    let (mut server, proxy_server) = pair();
    let mut connection = Connection::new(proxy_client, proxy_server, &[]).unwrap();
    join(&mut client, &mut server, &mut connection, &names);
    let mut observed = Vec::new();
    server.send(game_clientbound::NAMES, "login", &42_i32.to_be_bytes());
    let mut abilities = vec![0x06];
    abilities.extend_from_slice(&0.05_f32.to_be_bytes());
    abilities.extend_from_slice(&0.1_f32.to_be_bytes());
    server.send(game_clientbound::NAMES, "player_abilities", &abilities);
    pump(&mut connection, &names, &mut observed);

    let at_client = client.received();
    let ping = id(game_clientbound::NAMES, "ping");
    let (_, probe) = at_client
        .iter()
        .find(|(packet, _)| *packet == ping)
        .expect("the proxy probes after the abilities");
    assert_eq!(
        at_client.last().map(|(packet, _)| *packet),
        Some(ping),
        "the probe follows the packet it brackets"
    );
    let mut position = Vec::new();
    for value in [1.5_f64, 64.0, -2.25] {
        position.extend_from_slice(&value.to_be_bytes());
    }
    position.push(0x01);
    client.send(game_serverbound::NAMES, "move_player_pos", &position);
    client.send(game_serverbound::NAMES, "client_tick_end", &[]);
    client.send(game_serverbound::NAMES, "pong", probe);
    pump(&mut connection, &names, &mut observed);

    let at_server: Vec<i32> = server
        .received()
        .iter()
        .map(|(packet, _)| *packet)
        .collect();
    assert!(
        !at_server.contains(&id(game_serverbound::NAMES, "pong")),
        "the answer to a probe is not the server's"
    );
    assert!(at_server.contains(&id(game_serverbound::NAMES, "client_tick_end")));
    let events: Vec<&Event> = observed
        .iter()
        .filter_map(|item| match item {
            Observed::Applied(_, event) | Observed::Sent(_, event) => Some(event),
            Observed::Loaded => None,
        })
        .collect();
    assert_eq!(
        events,
        [
            &Event::Move(Move {
                position: Some([1.5, 64.0, -2.25]),
                rotation: None,
                on_ground: true,
                horizontal_collision: false,
            }),
            &Event::TickEnd,
            &Event::Abilities(Abilities {
                flying: true,
                may_fly: true,
                flying_speed: 0.05,
            }),
        ]
    );
}

#[test]
fn every_scenario_parses() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let scenario: Scenario = text
            .parse()
            .unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
        let (min, max) = (scenario.arena.min, scenario.arena.max);
        let volume: i64 = (0..3)
            .map(|axis| i64::from(max[axis] - min[axis] + 1))
            .product();
        assert!(
            volume <= 32768,
            "{}: /fill takes at most 32768 blocks",
            path.display()
        );
    }
}

#[test]
fn a_delayed_item_waits_half_its_round_trip_and_never_passes_an_earlier_one() {
    let samples = parse_trace("# a stall\n400\n\n20 # then a short trip\n").unwrap();
    let mut delay = Delay::new(samples);
    let start = Instant::now();
    delay.push(start, 1);
    delay.push(start, 2);
    let at = |millis| start + Duration::from_millis(millis);
    assert_eq!(delay.pop_due(at(199)), None, "the first item waits 200 ms");
    assert_eq!(delay.pop_due(at(200)), Some(1));
    assert_eq!(
        delay.pop_due(at(200)),
        Some(2),
        "the second item is due at 10 ms, but the connection keeps the order"
    );
}
