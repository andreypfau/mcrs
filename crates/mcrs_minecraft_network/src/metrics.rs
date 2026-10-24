use bevy_ecs::resource::Resource;
use mcrs_minecraft_protocol::Text;
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

type Check = fn(i32, &mut &[u8]) -> mcrs_minecraft_protocol::Result<Decoded<()>>;

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
        self.failures.get(row).copied().unwrap_or(0)
    }

    pub fn failures_total(&self) -> u64 {
        self.failures.iter().sum()
    }

    pub fn unknown(&self) -> u64 {
        self.unknown
    }

    /// Counts the frame when it fails to decode or names no row, the frames the game's
    /// decoder rejects. A row without a structure counts nothing.
    pub fn record(&mut self, id: i32, body: &[u8]) -> Result<(), Malformed> {
        let mut r = body;
        let (row, cause, detail) = match (self.check)(id, &mut r) {
            Ok(Decoded::Packet(())) if r.is_empty() => return Ok(()),
            Ok(Decoded::NotImplemented(_)) => return Ok(()),
            Ok(Decoded::Unknown(id)) => (
                None,
                format!("{DECODER_EXCEPTION}: Received unknown packet id {id}"),
                format!("unknown id {id}"),
            ),
            Ok(Decoded::Packet(())) => {
                let packet = self.packet_type(id);
                let protocol = protocol_id(self.table.state);
                let extra = r.len();
                (
                    Some(id),
                    // chisle: the game also names the packet's Java class in parentheses
                    // after the type; no table here knows those names.
                    format!(
                        "{DECODER_EXCEPTION}: java.io.IOException: Packet {protocol}/{packet} was larger than I expected, found {extra} bytes extra whilst reading packet {packet}"
                    ),
                    format!("{}: {extra} bytes left over", self.name(id)),
                )
            }
            Err(error) => (
                Some(id),
                format!(
                    "{DECODER_EXCEPTION}: Failed to decode packet '{}'",
                    self.packet_type(id)
                ),
                format!("{}: {error:#}", self.name(id)),
            ),
        };
        let count = match row {
            None => Some(&mut self.unknown),
            Some(id) => usize::try_from(id)
                .ok()
                .and_then(|row| self.failures.get_mut(row)),
        };
        let first = count.is_some_and(|count| {
            *count += 1;
            *count == 1
        });
        Err(Malformed {
            cause,
            log: first.then(|| format!("inbound packet: {:?} {detail}", self.table.state)),
        })
    }

    fn name(&self, id: i32) -> &'static str {
        usize::try_from(id)
            .ok()
            .and_then(|row| self.table.names.get(row))
            .copied()
            .unwrap_or_default()
    }

    fn packet_type(&self, id: i32) -> String {
        format!("serverbound/minecraft:{}", self.name(id))
    }
}

const DECODER_EXCEPTION: &str = "io.netty.handler.codec.DecoderException";

fn protocol_id(state: mcrs_minecraft_protocol::ConnectionState) -> &'static str {
    use mcrs_minecraft_protocol::ConnectionState;
    match state {
        ConnectionState::Handshaking => "handshake",
        ConnectionState::Status => "status",
        ConnectionState::Login => "login",
        ConnectionState::Configuration => "configuration",
        ConnectionState::Game => "play",
    }
}

/// A frame the game's decoder rejects, which ends the connection that sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed {
    /// The exception the game's decoder throws for the frame, as its connection prints it.
    pub cause: String,
    /// Present on the first increment of the frame's counter, so a client cannot choose how
    /// often anything is logged.
    pub log: Option<String>,
}

impl Malformed {
    pub fn reason(&self) -> Text {
        Text::translate(
            "disconnect.genericReason",
            [Text::text(format!("Internal Exception: {}", self.cause))],
        )
    }
}

/// Written only by `bridge_inbound`.
#[derive(Resource, Debug)]
pub struct GameDecodeCounts(TableCounts);

impl GameDecodeCounts {
    pub fn counts(&self) -> &TableCounts {
        &self.0
    }

    pub fn record(&mut self, id: i32, body: &[u8]) -> Result<(), Malformed> {
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

    pub fn record(
        &mut self,
        state: ConnectionState,
        id: i32,
        body: &[u8],
    ) -> Result<(), Malformed> {
        match state {
            ConnectionState::Login => self.login.record(id, body),
            ConnectionState::Configuration => self.configuration.record(id, body),
            ConnectionState::Game => Ok(()),
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

    fn log(counts: &mut GameDecodeCounts, id: i32, body: &[u8]) -> Option<String> {
        counts
            .record(id, body)
            .expect_err("the frame is rejected")
            .log
    }

    #[test]
    fn every_rejected_frame_fails_and_only_the_first_increment_of_a_counter_is_logged() {
        let mut counts = GameDecodeCounts::default();
        let keep_alive = row_of(game_serverbound::NAMES, "keep_alive").unwrap();

        let line = log(&mut counts, keep_alive, &[1, 2]).expect("first failure is logged");
        assert!(line.starts_with("inbound packet: "), "{line}");
        assert!(line.contains("keep_alive"), "{line}");
        assert_eq!(log(&mut counts, keep_alive, &[1, 2]), None);
        assert_eq!(counts.counts().failures(keep_alive as usize), 2);

        let past = game_serverbound::NAMES.len() as i32;
        let line = log(&mut counts, past, &[]).expect("first unknown id is logged");
        assert!(line.starts_with("inbound packet: "), "{line}");
        assert!(line.contains(&past.to_string()), "{line}");
        assert_eq!(log(&mut counts, -1, &[]), None);
        assert_eq!(counts.counts().unknown(), 2);
    }

    #[test]
    fn leftover_bytes_are_a_failure_of_the_row() {
        let mut counts = GameDecodeCounts::default();
        let keep_alive = row_of(game_serverbound::NAMES, "keep_alive").unwrap();
        assert_eq!(counts.record(keep_alive, &[0; 8]), Ok(()));
        let line = log(&mut counts, keep_alive, &[0; 9]).unwrap();
        assert!(line.contains("1 bytes left over"), "{line}");
    }
}
