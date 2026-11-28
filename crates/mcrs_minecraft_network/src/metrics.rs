use bevy_ecs::resource::Resource;
use mcrs_minecraft_nbt::{NbtBudget, NbtQuotaExceeded};
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

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeTelemetry {
    pub kick_overflow_total: u64,
    pub kick_flood_total: u64,
    pub outbound_messages_consumed_total: u64,
    pub outbound_no_queue_total: u64,
    pub encode_calls_total: u64,
    pub shared_appends_total: u64,
    pub shared_bytes_total: u64,
    pub shared_frames_total: u64,
    pub framed_appends_total: u64,
    pub replication_bytes_total: u64,
    pub packets: PacketKinds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    EntityMove,
    PlayerInfo,
    Replication,
    Chunk,
    Other,
}

impl PacketKind {
    pub const ALL: [Self; 5] = [
        Self::EntityMove,
        Self::PlayerInfo,
        Self::Replication,
        Self::Chunk,
        Self::Other,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::EntityMove => "move",
            Self::PlayerInfo => "player info",
            Self::Replication => "muxer",
            Self::Chunk => "chunk",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PacketKinds([u64; PacketKind::ALL.len()]);

impl PacketKinds {
    pub fn count(&mut self, kind: PacketKind) {
        self.add(kind, 1);
    }

    pub fn add(&mut self, kind: PacketKind, packets: u64) {
        if let Some(slot) = self.0.get_mut(kind as usize) {
            *slot = slot.saturating_add(packets);
        }
    }

    #[must_use]
    pub fn get(&self, kind: PacketKind) -> u64 {
        self.0.get(kind as usize).copied().unwrap_or(0)
    }
}

type Check =
    fn(i32, &mut &[u8], &mut Triggers) -> mcrs_minecraft_protocol::Result<Decoded<Trigger>>;

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
                    format!(
                        "{DECODER_EXCEPTION}: java.io.IOException: Packet {protocol}/{packet} was larger than I expected, found {extra} bytes extra whilst reading packet {packet}"
                    ),
                    format!("{}: {extra} bytes left over", self.name(id)),
                )
            }
            Err(error) => (
                Some(id),
                self.decode_failure(id),
                format!("{}: {error:#}", self.name(id)),
            ),
        };
        Err(self.fail(row, cause, detail))
    }

    fn fail(&mut self, row: Option<i32>, cause: String, detail: String) -> Malformed {
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
        Malformed {
            cause,
            log: first.then(|| format!("inbound packet: {:?} {detail}", self.table.state)),
        }
    }

    fn decode_failure(&self, id: i32) -> String {
        format!(
            "{DECODER_EXCEPTION}: Failed to decode packet '{}'",
            self.packet_type(id)
        )
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed {
    pub cause: String,
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

#[derive(Resource, Debug)]
pub struct PacketDispatcher {
    side: PacketSide,
    tables: Box<[TableCounts]>,
    nbt_budget: NbtBudget,
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
            nbt_budget: NbtBudget::default(),
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
            nbt_budget: NbtBudget::default(),
        }
    }

    pub fn defer<R: Row>(mut self) -> Self {
        type P<R> = <R as Row>::Packet<'static>;
        if P::<R>::SIDE == self.side
            && let Some(counts) = self.counts_mut(P::<R>::STATE)
        {
            counts.deferred.push((P::<R>::ID, Trigger::of::<R>()));
        }
        self
    }

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

    pub(crate) fn set_nbt_budget(&mut self, budget: NbtBudget) {
        self.nbt_budget = budget;
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

    pub fn dispatch(
        &mut self,
        state: ConnectionState,
        id: i32,
        body: &[u8],
    ) -> Result<Option<Trigger>, Malformed> {
        let (side, budget) = (self.side, self.nbt_budget);
        match self.counts_mut(state) {
            Some(counts) if side == PacketSide::Serverbound => {
                let mut recorded = Ok(None);
                let fits = mcrs_minecraft_nbt::with_nbt_budget(budget, || {
                    recorded = counts.record(id, body);
                })
                .is_ok();
                match recorded {
                    Ok(_) if !fits => {
                        let cause = counts.decode_failure(id);
                        let detail = format!("{}: {NbtQuotaExceeded}", counts.name(id));
                        Err(counts.fail(Some(id), cause, detail))
                    }
                    recorded => recorded,
                }
            }
            Some(counts) => counts.record(id, body),
            None => Err(Malformed {
                cause: format!("{DECODER_EXCEPTION}: Received unknown packet id {id}"),
                log: None,
            }),
        }
    }

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

    fn two_nbt(
        id: i32,
        r: &mut &[u8],
        _: &mut Triggers,
    ) -> mcrs_minecraft_protocol::Result<Decoded<Trigger>> {
        use mcrs_minecraft_nbt::tag::NbtTag;
        use mcrs_minecraft_protocol::Decode;
        let first = NbtTag::decode(r);
        let second = NbtTag::decode(r);
        if id == 0 {
            first?;
            second?;
        }
        Ok(Decoded::NotImplemented("two_nbt"))
    }

    fn two_arrays(length: i32) -> Vec<u8> {
        let mut array = vec![mcrs_minecraft_nbt::BYTE_ARRAY_ID];
        array.extend_from_slice(&length.to_be_bytes());
        array.resize(array.len() + length as usize, 0);
        [array.clone(), array].concat()
    }

    #[test]
    fn the_network_nbt_of_one_packet_shares_one_budget_and_a_refusal_is_logged_once() {
        let mut counts = PacketDispatcher::serverbound()
            .with_play(PlayTable::new(&game_serverbound::TABLE, two_nbt));
        counts.set_nbt_budget(NbtBudget {
            bytes: 2_097_152,
            tags: 262_144,
        });
        let fits = two_arrays(1_048_576);
        let over = two_arrays(1_048_577);
        for _ in 0..2 {
            assert!(
                matches!(counts.dispatch(ConnectionState::Game, 0, &fits), Ok(None)),
                "each packet has a budget of its own"
            );
        }
        let refused = counts
            .dispatch(ConnectionState::Game, 0, &over)
            .expect_err("the packet is past the byte limit");
        assert!(
            refused.cause.starts_with(DECODER_EXCEPTION),
            "{}",
            refused.cause
        );
        assert!(
            refused.log.is_some(),
            "the first failure of a row is logged"
        );
        let again = counts
            .dispatch(ConnectionState::Game, 0, &over)
            .expect_err("the packet is past the byte limit");
        assert_eq!(again.log, None);

        let swallowed = counts
            .dispatch(ConnectionState::Game, 1, &over)
            .expect_err("a swallowed quota error is still a refusal");
        assert!(
            swallowed.cause.starts_with(DECODER_EXCEPTION),
            "{}",
            swallowed.cause
        );
        let line = swallowed.log.expect("the first failure of a row is logged");
        assert!(line.contains("exceeds its budget"), "{line}");
        assert_eq!(
            counts
                .dispatch(ConnectionState::Game, 1, &over)
                .expect_err("a swallowed quota error is still a refusal")
                .log,
            None
        );
        let failures = counts.counts(ConnectionState::Game).unwrap().failures(1);
        assert_eq!(failures, 2, "each refusal is counted under its row");

        counts.set_nbt_budget(NbtBudget {
            bytes: 1_000_000,
            tags: 262_144,
        });
        assert!(
            counts.dispatch(ConnectionState::Game, 0, &fits).is_err(),
            "the plugin byte limit applies"
        );
    }
}
