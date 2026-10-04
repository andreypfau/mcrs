use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_protocol::packets::table::TABLES;
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
