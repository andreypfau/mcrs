use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_protocol::packets::table::{Decoded, TABLES, Table, game_serverbound};
use mcrs_minecraft_protocol::{ConnectionState, PacketSide};
use serde::Deserialize;

#[derive(Deserialize)]
struct Row {
    protocol_id: i32,
}

type Report = BTreeMap<String, BTreeMap<String, BTreeMap<String, Row>>>;

fn report() -> Report {
    serde_json::from_str(include_str!("../../../../assets/mcrs/reports/packets.json")).unwrap()
}

fn state_key(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Handshaking => "handshake",
        ConnectionState::Status => "status",
        ConnectionState::Login => "login",
        ConnectionState::Configuration => "configuration",
        ConnectionState::Game => "play",
    }
}

fn side_key(side: PacketSide) -> &'static str {
    match side {
        PacketSide::Clientbound => "clientbound",
        PacketSide::Serverbound => "serverbound",
    }
}

#[test]
fn each_table_equals_the_report_in_names_and_order() {
    let report = report();
    for table in TABLES {
        let rows = &report[state_key(table.state)][side_key(table.side)];
        let mut by_id: Vec<(i32, &str)> = rows
            .iter()
            .map(|(name, row)| (row.protocol_id, name.strip_prefix("minecraft:").unwrap()))
            .collect();
        by_id.sort();
        let ids: Vec<i32> = by_id.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            (0..by_id.len() as i32).collect::<Vec<_>>(),
            "{} ids in the report",
            table.name
        );
        let names: Vec<&str> = by_id.iter().map(|(_, name)| *name).collect();
        assert_eq!(names, table.names, "{}", table.name);
    }
}

#[test]
fn the_tables_are_the_reports_tables() {
    let from_report: BTreeSet<(String, String)> = report()
        .into_iter()
        .flat_map(|(state, sides)| sides.into_keys().map(move |side| (state.clone(), side)))
        .collect();
    let mut from_tables: Vec<(String, String)> = TABLES
        .iter()
        .map(|table| {
            (
                state_key(table.state).to_owned(),
                side_key(table.side).to_owned(),
            )
        })
        .collect();
    from_tables.sort();
    assert_eq!(from_tables, from_report.into_iter().collect::<Vec<_>>());
}

macro_rules! collect_tables {
    ($($table:ident: $enum:ident $(<$tlt:lifetime>)?, $state:ident, $side:ident in ($($module:tt)*) {
        $($name:literal $(=> $ty:ident $(<$lt:lifetime>)?)?,)*
    })*) => {
        fn expanded() -> Vec<(&'static str, Vec<&'static str>)> {
            vec![$((stringify!($table), vec![$($name),*])),*]
        }
    };
}

mcrs_minecraft_protocol::for_each_packet_table!(collect_tables);

#[test]
fn another_crate_expands_the_row_list() {
    let expanded = expanded();
    assert_eq!(expanded.len(), TABLES.len());
    for ((ident, names), table) in expanded.iter().zip(TABLES) {
        assert_eq!(*ident, table.name);
        assert_eq!(names.as_slice(), table.names, "{ident}");
    }
}


#[derive(Debug)]
enum Outcome {
    Packet,
    NotImplemented { name: &'static str, left: usize },
    Unknown(i32),
    Failed(String),
}

fn outcome<P>(result: anyhow::Result<Decoded<P>>, left: usize) -> Outcome {
    match result {
        Ok(Decoded::Packet(_)) => Outcome::Packet,
        Ok(Decoded::NotImplemented(name)) => Outcome::NotImplemented { name, left },
        Ok(Decoded::Unknown(id)) => Outcome::Unknown(id),
        Err(error) => Outcome::Failed(format!("{error:#}")),
    }
}

macro_rules! decode_each_id {
    ($($table:ident: $enum:ident $(<$tlt:lifetime>)?, $state:ident, $side:ident in ($($module:tt)*) {
        $($name:literal $(=> $ty:ident $(<$lt:lifetime>)?)?,)*
    })*) => {
        fn decode_each_id(body: &[u8]) -> Vec<(&'static str, i32, Outcome)> {
            let mut outcomes = Vec::new();
            $(
                for id in -1..=mcrs_minecraft_protocol::packets::table::$table::NAMES.len() as i32 {
                    let mut r = body;
                    let result = mcrs_minecraft_protocol::packets::table::$table::decode(id, &mut r);
                    outcomes.push((stringify!($table), id, outcome(result, r.len())));
                }
            )*
            outcomes
        }
    };
}

mcrs_minecraft_protocol::for_each_packet_table!(decode_each_id);

fn table_named(name: &str) -> &'static Table {
    TABLES.iter().find(|table| table.name == name).unwrap()
}

#[test]
fn a_typed_id_decodes_to_its_packet() {
    let table = table_named("game_serverbound");
    let id = table.names.iter().position(|n| *n == "keep_alive").unwrap() as i32;
    let body = 7i64.to_be_bytes();
    let mut r = &body[..];
    match game_serverbound::decode(id, &mut r).unwrap() {
        Decoded::Packet(game_serverbound::ServerboundGamePacket::ServerboundKeepAlive(alive)) => {
            assert_eq!(alive.0.payload, 7)
        }
        other => panic!("not the keep alive packet: {other:?}"),
    }
    assert!(r.is_empty());
}

#[test]
fn a_named_row_is_known_and_its_body_is_discarded() {
    let table = table_named("game_serverbound");
    let row = table
        .names
        .iter()
        .position(|name| !table.typed.contains(name))
        .expect("a game_serverbound row without a structure");
    let body = [1u8, 2, 3, 4];
    let mut r = &body[..];
    match game_serverbound::decode(row as i32, &mut r).unwrap() {
        Decoded::NotImplemented(name) => assert_eq!(name, table.names[row]),
        other => panic!("not a known but unimplemented row: {other:?}"),
    }
    assert!(r.is_empty());
}

#[test]
fn an_id_outside_the_table_is_unknown() {
    let rows = table_named("game_serverbound").names.len() as i32;
    for id in [-1, rows, rows + 1, i32::MAX, i32::MIN] {
        let body = [9u8, 9];
        let mut r = &body[..];
        match game_serverbound::decode(id, &mut r).unwrap() {
            Decoded::Unknown(unknown) => assert_eq!(unknown, id),
            other => panic!("id {id} is not unknown: {other:?}"),
        }
        assert_eq!(r.len(), 2, "an unknown id leaves the body alone");
    }
}

#[test]
fn every_row_of_every_table_has_an_outcome() {
    let body = [0u8; 3];
    for (table, id, outcome) in decode_each_id(&body) {
        let names = table_named(table).names;
        let typed = table_named(table).typed;
        let in_table = usize::try_from(id).ok().filter(|row| *row < names.len());
        match (in_table, &outcome) {
            (None, Outcome::Unknown(unknown)) => assert_eq!(*unknown, id, "{table}"),
            (None, other) => panic!("{table} id {id} is outside the table but is {other:?}"),
            (Some(row), Outcome::NotImplemented { name, left }) => {
                assert_eq!(*name, names[row], "{table}");
                assert!(!typed.contains(name), "{table} {name} has a structure");
                assert_eq!(*left, 0, "{table} {name} leaves its body");
            }
            (Some(row), Outcome::Packet) => {
                assert!(typed.contains(&names[row]), "{table} {}", names[row]);
            }
            (Some(row), Outcome::Failed(error)) => {
                assert!(typed.contains(&names[row]), "{table} {}", names[row]);
                assert!(error.contains(names[row]), "{table}: {error}");
            }
            (Some(_), Outcome::Unknown(_)) => panic!("{table} id {id} is a row but is unknown"),
        }
    }
}

#[test]
fn a_truncated_typed_body_is_an_error_naming_the_packet() {
    let table = table_named("game_serverbound");
    let id = table.names.iter().position(|n| *n == "keep_alive").unwrap() as i32;
    let body = 7i64.to_be_bytes();
    let mut r = &body[..3];
    let error = game_serverbound::decode(id, &mut r).unwrap_err();
    assert!(format!("{error:#}").contains("keep_alive"), "{error:#}");
}

#[test]
fn no_body_makes_a_decode_panic() {
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut bodies: Vec<Vec<u8>> = vec![vec![0xff; 64], vec![0x00; 64], vec![0x7f; 64]];
    for length in 0..40usize {
        for _ in 0..16 {
            bodies.push((0..length).map(|_| next() as u8).collect());
        }
    }
    for body in &bodies {
        decode_each_id(body);
    }
}

#[test]
fn a_table_header_names_its_enum_after_its_side_and_state() {
    for table in TABLES {
        let state = match table.state {
            ConnectionState::Handshaking => "Handshake".to_owned(),
            other => format!("{other:?}"),
        };
        assert_eq!(
            table.enum_name,
            format!("{:?}{state}Packet", table.side),
            "{}",
            table.name
        );
    }
}

#[test]
fn a_deeply_nested_click_action_payload_is_refused_and_does_not_overflow() {
    let table = table_named("game_serverbound");
    let id = table
        .names
        .iter()
        .position(|n| *n == "custom_click_action")
        .unwrap() as i32;
    let mut tag = vec![9u8];
    for _ in 0..4000 {
        tag.extend_from_slice(&[9, 0, 0, 0, 1]);
    }
    tag.extend_from_slice(&[0, 0, 0, 0, 0]);
    assert!(tag.len() < 65536);
    let mut body = vec![3u8];
    body.extend_from_slice(b"a:b");
    body.extend_from_slice(&[(tag.len() & 0x7f) as u8 | 0x80, (tag.len() >> 7) as u8]);
    body.extend_from_slice(&tag);
    let mut r = &body[..];
    let error = game_serverbound::decode(id, &mut r).unwrap_err();
    assert!(
        format!("{error:#}").contains("custom_click_action"),
        "{error:#}"
    );
}
