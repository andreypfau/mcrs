use bevy_ecs::resource::Resource;
use mcrs_minecraft_protocol::packets::table::{
    Decoded, Table, configuration_serverbound, game_serverbound, login_serverbound,
};

use crate::ConnectionState;

/// What the bridge has shed, kicked and routed since the server started.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeTelemetry {
    pub drop_normal_total: u64,
    pub drop_low_total: u64,
    pub kick_overflow_total: u64,
    pub kick_flood_total: u64,
    pub outbound_messages_consumed_total: u64,
    pub encode_unhandled_total: u64,
    pub outbound_no_queue_total: u64,
}

type Check = fn(i32, &mut &[u8]) -> anyhow::Result<Decoded<()>>;

/// Decode failures per row and ids outside the table, for one serverbound table.
/// Sized by the table when built; nothing a client sends grows it.
#[derive(Debug)]
pub struct TableCounts {
    table: &'static Table,
    check: Check,
    failures: Box<[u64]>,
    unknown: u64,
}

impl TableCounts {
    fn new(table: &'static Table, check: Check) -> Self {
        Self {
            table,
            check,
            failures: vec![0; table.names.len()].into_boxed_slice(),
            unknown: 0,
        }
    }

    pub fn failures(&self, row: usize) -> u64 {
        self.failures[row]
    }

    pub fn failures_total(&self) -> u64 {
        self.failures.iter().sum()
    }

    pub fn unknown(&self) -> u64 {
        self.unknown
    }

    /// Counts the frame when it fails to decode or names no row. A row without a
    /// structure counts nothing. Answers the line to log when this is the first
    /// increment of its counter, so a client cannot choose how often anything is logged.
    pub fn record(&mut self, id: i32, body: &[u8]) -> Option<String> {
        let mut r = body;
        match (self.check)(id, &mut r) {
            Ok(Decoded::Packet(())) if r.is_empty() => None,
            Ok(Decoded::Packet(())) => {
                self.fail(id, anyhow::anyhow!("{} bytes left over", r.len()))
            }
            Ok(Decoded::NotImplemented(_)) => None,
            Ok(Decoded::Unknown(id)) => {
                self.unknown += 1;
                (self.unknown == 1)
                    .then(|| format!("inbound packet: {:?} unknown id {id}", self.table.state))
            }
            Err(error) => self.fail(id, error),
        }
    }

    fn fail(&mut self, id: i32, error: anyhow::Error) -> Option<String> {
        let row = usize::try_from(id).ok()?;
        let count = self.failures.get_mut(row)?;
        *count += 1;
        (*count == 1).then(|| {
            format!(
                "inbound packet: {:?} {}: {error:#}",
                self.table.state, self.table.names[row]
            )
        })
    }
}

/// Written only by `bridge_inbound`.
#[derive(Resource, Debug)]
pub struct GameDecodeCounts(TableCounts);

impl GameDecodeCounts {
    pub fn counts(&self) -> &TableCounts {
        &self.0
    }

    pub fn record(&mut self, id: i32, body: &[u8]) -> Option<String> {
        self.0.record(id, body)
    }
}

impl Default for GameDecodeCounts {
    fn default() -> Self {
        Self(TableCounts::new(&game_serverbound::TABLE, |id, r| {
            game_serverbound::decode(id, r).map(Decoded::forget)
        }))
    }
}

/// Written only by `run_event_loop`.
#[derive(Resource, Debug)]
pub struct PreGameDecodeCounts {
    login: TableCounts,
    configuration: TableCounts,
}

impl PreGameDecodeCounts {
    pub fn counts(&self, state: ConnectionState) -> Option<&TableCounts> {
        match state {
            ConnectionState::Login => Some(&self.login),
            ConnectionState::Configuration => Some(&self.configuration),
            ConnectionState::Game => None,
        }
    }

    pub fn record(&mut self, state: ConnectionState, id: i32, body: &[u8]) -> Option<String> {
        match state {
            ConnectionState::Login => self.login.record(id, body),
            ConnectionState::Configuration => self.configuration.record(id, body),
            ConnectionState::Game => None,
        }
    }
}

impl Default for PreGameDecodeCounts {
    fn default() -> Self {
        Self {
            login: TableCounts::new(&login_serverbound::TABLE, |id, r| {
                login_serverbound::decode(id, r).map(Decoded::forget)
            }),
            configuration: TableCounts::new(&configuration_serverbound::TABLE, |id, r| {
                configuration_serverbound::decode(id, r).map(Decoded::forget)
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::packets::table::row_of;

    #[test]
    fn only_the_first_increment_of_a_counter_is_logged() {
        let mut counts = GameDecodeCounts::default();
        let keep_alive = row_of(game_serverbound::NAMES, "keep_alive");

        let line = counts
            .record(keep_alive, &[1, 2])
            .expect("first failure is logged");
        assert!(line.starts_with("inbound packet: "), "{line}");
        assert!(line.contains("keep_alive"), "{line}");
        assert_eq!(counts.record(keep_alive, &[1, 2]), None);
        assert_eq!(counts.counts().failures(keep_alive as usize), 2);

        let past = game_serverbound::NAMES.len() as i32;
        let line = counts
            .record(past, &[])
            .expect("first unknown id is logged");
        assert!(line.starts_with("inbound packet: "), "{line}");
        assert!(line.contains(&past.to_string()), "{line}");
        assert_eq!(counts.record(-1, &[]), None);
        assert_eq!(counts.counts().unknown(), 2);
    }

    #[test]
    fn leftover_bytes_are_a_failure_of_the_row() {
        let mut counts = GameDecodeCounts::default();
        let keep_alive = row_of(game_serverbound::NAMES, "keep_alive");
        assert_eq!(counts.record(keep_alive, &[0; 8]), None);
        let line = counts.record(keep_alive, &[0; 9]).unwrap();
        assert!(line.contains("1 bytes left over"), "{line}");
    }
}
