use bevy_ecs::resource::Resource;
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundFinishConfiguration;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundStartConfiguration;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginFinished;
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundLoginAcknowledged;
use mcrs_minecraft_protocol::packets::table::{
    Decoded, Row, Table, configuration_clientbound, configuration_serverbound, game_clientbound,
    game_serverbound, handshake_serverbound, login_clientbound, login_serverbound,
    status_clientbound, status_serverbound,
};
use mcrs_minecraft_protocol::{ConnectionState, Packet, PacketSide};
use mcrs_minecraft_text::Text;
use tracing::trace;

use crate::event::{Trigger, Triggers};

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
    /// Packets serialized for one recipient.
    pub encode_calls_total: u64,
    /// Packets serialized once and appended to a recipient as shared bytes.
    pub shared_appends_total: u64,
    /// Bytes of the shared appends, one count for each recipient.
    pub shared_bytes_total: u64,
    /// Frames made once for a shared packet and a compression threshold: each compresses the
    /// packet once at most.
    pub shared_frames_total: u64,
    /// Shared appends that went as a frame made once, not framed again by the writer.
    pub framed_appends_total: u64,
    pub packets: PacketKinds,
}

/// What an outbound packet is for, as far as replication cost goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    EntityMove,
    Spawn,
    Remove,
    PlayerInfo,
    /// A packet of the entity muxer: a spawn, a move, a removal or a player list update, which
    /// the bridge counts without reading its id.
    Replication,
    Chunk,
    Other,
}

/// Packets handed to connections, by [`PacketKind`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PacketKinds {
    pub entity_move: u64,
    pub spawn: u64,
    pub remove: u64,
    pub player_info: u64,
    pub replication: u64,
    pub chunk: u64,
    pub other: u64,
}

impl PacketKinds {
    pub fn count(&mut self, kind: PacketKind) {
        self.add(kind, 1);
    }

    pub fn add(&mut self, kind: PacketKind, packets: u64) {
        let slot = match kind {
            PacketKind::EntityMove => &mut self.entity_move,
            PacketKind::Spawn => &mut self.spawn,
            PacketKind::Remove => &mut self.remove,
            PacketKind::PlayerInfo => &mut self.player_info,
            PacketKind::Replication => &mut self.replication,
            PacketKind::Chunk => &mut self.chunk,
            PacketKind::Other => &mut self.other,
        };
        *slot = slot.saturating_add(packets);
    }

    pub fn since(self, earlier: Self) -> Self {
        Self {
            entity_move: self.entity_move.saturating_sub(earlier.entity_move),
            spawn: self.spawn.saturating_sub(earlier.spawn),
            remove: self.remove.saturating_sub(earlier.remove),
            player_info: self.player_info.saturating_sub(earlier.player_info),
            replication: self.replication.saturating_sub(earlier.replication),
            chunk: self.chunk.saturating_sub(earlier.chunk),
            other: self.other.saturating_sub(earlier.other),
        }
    }
}

type Check =
    fn(i32, &mut &[u8], &mut Triggers) -> mcrs_minecraft_protocol::Result<Decoded<Trigger>>;

/// The play-state table of one side, typed by a crate above this one. The dispatcher holds the
/// rows of this crate's own play packets until one replaces them.
#[derive(Clone, Copy, Debug)]
pub struct PlayTable {
    table: &'static Table,
    check: Check,
}

impl PlayTable {
    pub const fn new(table: &'static Table, check: Check) -> Self {
        Self { table, check }
    }
}

/// Decode failures per row and ids outside the table, for one table.
/// Sized by the table when built; nothing a peer sends grows it.
#[derive(Debug)]
pub struct TableCounts {
    table: &'static Table,
    check: Check,
    failures: Box<[u64]>,
    unknown: u64,
    deferred: Vec<(i32, Trigger)>,
}

impl TableCounts {
    fn new(table: &'static Table, check: Check) -> Self {
        Self {
            table,
            check,
            failures: vec![0; table.names.len()].into_boxed_slice(),
            unknown: 0,
            deferred: Vec::new(),
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
    /// decoder rejects. A row without a structure counts nothing and has no trigger.
    pub fn record(&mut self, id: i32, body: &[u8]) -> Result<Option<Trigger>, Malformed> {
        if let Some(&(_, trigger)) = self.deferred.iter().find(|(row, _)| *row == id) {
            return Ok(Some(trigger));
        }
        let mut r = body;
        let (row, cause, detail) = match (self.check)(id, &mut r, &mut Triggers) {
            Ok(Decoded::Packet(trigger)) if r.is_empty() => return Ok(Some(trigger)),
            Ok(Decoded::NotImplemented(name)) => {
                trace!(
                    "inbound packet: {:?} {name} has no structure",
                    self.table.state
                );
                return Ok(None);
            }
            Ok(Decoded::Unknown(id)) => (
                None,
                format!("{DECODER_EXCEPTION}: Received unknown packet id {id}"),
                format!("unknown id {id}"),
            ),
            Ok(Decoded::Packet(_)) => {
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
        let side = match self.table.side {
            PacketSide::Clientbound => "clientbound",
            PacketSide::Serverbound => "serverbound",
        };
        format!("{side}/minecraft:{}", self.name(id))
    }
}

const DECODER_EXCEPTION: &str = "io.netty.handler.codec.DecoderException";

fn protocol_id(state: ConnectionState) -> &'static str {
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

macro_rules! counts {
    ($($table:ident),*) => {
        vec![$(TableCounts::new(&$table::TABLE, $table::visit::<Triggers>),)*]
            .into_boxed_slice()
    };
}

/// The one place a received frame is checked: it picks the table of the connection's state,
/// decodes the frame once and hands back the trigger of the frame's typed event.
#[derive(Resource, Debug)]
pub struct PacketDispatcher {
    side: PacketSide,
    tables: Box<[TableCounts]>,
}

impl PacketDispatcher {
    pub fn serverbound() -> Self {
        Self {
            side: PacketSide::Serverbound,
            tables: counts!(
                handshake_serverbound,
                status_serverbound,
                login_serverbound,
                configuration_serverbound,
                game_serverbound
            ),
        }
    }

    pub fn clientbound() -> Self {
        Self {
            side: PacketSide::Clientbound,
            tables: counts!(
                status_clientbound,
                login_clientbound,
                configuration_clientbound,
                game_clientbound
            ),
        }
    }

    /// Leaves the check of a row to its observer, which decodes the frame off the main thread
    /// and drops the connection when the frame does not decode.
    pub fn defer<R: Row>(mut self) -> Self {
        type P<R> = <R as Row>::Packet<'static>;
        if P::<R>::SIDE == self.side
            && let Some(counts) = self.counts_mut(P::<R>::STATE)
        {
            counts.deferred.push((P::<R>::ID, Trigger::of::<R>()));
        }
        self
    }

    /// Types the play rows that `play` types, in place of this crate's own play table. The
    /// table must be of this dispatcher's side.
    pub fn with_play(mut self, play: PlayTable) -> Self {
        self.set_play(play);
        self
    }

    pub fn set_play(&mut self, play: PlayTable) {
        if play.table.side != self.side || play.table.state != ConnectionState::Game {
            return;
        }
        if let Some(counts) = self.counts_mut(ConnectionState::Game) {
            let deferred = std::mem::take(&mut counts.deferred);
            *counts = TableCounts::new(play.table, play.check);
            counts.deferred = deferred;
        }
    }

    pub fn counts(&self, state: ConnectionState) -> Option<&TableCounts> {
        self.tables
            .iter()
            .find(|counts| counts.table.state == state)
    }

    fn counts_mut(&mut self, state: ConnectionState) -> Option<&mut TableCounts> {
        self.tables
            .iter_mut()
            .find(|counts| counts.table.state == state)
    }

    /// A frame in a state without a table of this side names no row, as an unknown id does.
    pub fn dispatch(
        &mut self,
        state: ConnectionState,
        id: i32,
        body: &[u8],
    ) -> Result<Option<Trigger>, Malformed> {
        match self.counts_mut(state) {
            Some(counts) => counts.record(id, body),
            None => Err(Malformed {
                cause: format!("{DECODER_EXCEPTION}: Received unknown packet id {id}"),
                log: None,
            }),
        }
    }

    /// The frames behind one that ends its state are in the next state's protocol, and the
    /// state changes only once that frame is handled, so a reader stops after it.
    pub fn ends_state(&self, state: ConnectionState, id: i32) -> bool {
        ENDS_STATE.contains(&(self.side, state, id))
    }
}

const fn ends<P: Packet>() -> (PacketSide, ConnectionState, i32) {
    (P::SIDE, P::STATE, P::ID)
}

const ENDS_STATE: [(PacketSide, ConnectionState, i32); 7] = [
    ends::<ServerboundHandshake>(),
    ends::<ServerboundLoginAcknowledged>(),
    ends::<ServerboundFinishConfiguration>(),
    ends::<ServerboundConfigurationAcknowledged>(),
    ends::<ClientboundLoginFinished>(),
    ends::<ClientboundFinishConfiguration>(),
    ends::<ClientboundStartConfiguration>(),
];

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::packets::table::row_of;

    fn log(counts: &mut PacketDispatcher, id: i32, body: &[u8]) -> Option<String> {
        counts
            .dispatch(ConnectionState::Game, id, body)
            .expect_err("the frame is rejected")
            .log
    }

    #[test]
    fn every_rejected_frame_fails_and_only_the_first_increment_of_a_counter_is_logged() {
        let mut counts = PacketDispatcher::serverbound();
        let keep_alive = row_of(game_serverbound::NAMES, "keep_alive").unwrap();

        let line = log(&mut counts, keep_alive, &[1, 2]).expect("first failure is logged");
        assert!(line.starts_with("inbound packet: "), "{line}");
        assert!(line.contains("keep_alive"), "{line}");
        assert_eq!(log(&mut counts, keep_alive, &[1, 2]), None);
        assert_eq!(
            counts
                .counts(ConnectionState::Game)
                .unwrap()
                .failures(keep_alive as usize),
            2
        );

        let past = game_serverbound::NAMES.len() as i32;
        let line = log(&mut counts, past, &[]).expect("first unknown id is logged");
        assert!(line.starts_with("inbound packet: "), "{line}");
        assert!(line.contains(&past.to_string()), "{line}");
        assert_eq!(log(&mut counts, -1, &[]), None);
        assert_eq!(counts.counts(ConnectionState::Game).unwrap().unknown(), 2);
    }

    #[test]
    fn leftover_bytes_are_a_failure_of_the_row() {
        let mut counts = PacketDispatcher::serverbound();
        let keep_alive = row_of(game_serverbound::NAMES, "keep_alive").unwrap();
        assert!(
            counts
                .dispatch(ConnectionState::Game, keep_alive, &[0; 8])
                .is_ok_and(|trigger| trigger.is_some())
        );
        let line = log(&mut counts, keep_alive, &[0; 9]).unwrap();
        assert!(line.contains("1 bytes left over"), "{line}");
    }
}
