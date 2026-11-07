use std::collections::VecDeque;
use std::time::Duration;

use crate::probes::{Answer, Probes};
use crate::{Action, ChangeState, ChangeView, Checked, Disconnect, Finding, Report, Serverbound};

/// How far the balance of the tick rate check stays behind the newest answered probe.
const TICK_DRIFT: Duration = Duration::from_millis(120);
/// How long a probe can wait for its answer before the connection ends.
const MAX_ACK_DELAY: Duration = Duration::from_secs(60);
const CLIENT_TICK: Duration = Duration::from_millis(50);
/// The client sends a position at least once in this many client ticks.
const SILENT_LIMIT: u32 = 20;
/// A client that does not report its load counts as loaded this many server ticks after it
/// spawns.
const LOAD_TICKS: u32 = 60;

/// A predicted-state change between the probes that bracket it.
#[derive(Clone, Copy, Debug)]
struct Bracket {
    before: u64,
    after: Option<u64>,
}

/// Layer 1: the state of one connection, without a world, fed the packets of both directions
/// in the order of the connection. Times are durations since an origin that the caller keeps.
///
/// The writer calls [`Self::change`], [`Self::wrote`] and [`Self::end_server_tick`] as the
/// order on the wire becomes final, and writes each probe id they return as a `ping`.
#[derive(Clone, Debug, Default)]
pub struct MoveGuard {
    probes: Probes,
    /// The newest packet written is a probe.
    probe_last: bool,
    changes: VecDeque<Bracket>,
    first_change: u64,
    answered_at_tick: u64,
    client_tick: u64,
    moved: bool,
    positioned: bool,
    silent_ticks: u32,
    loaded: bool,
    server_ticks: u32,
    pending_teleport: Option<i32>,
    balance: Option<Duration>,
}

impl MoveGuard {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A predicted-state change goes out at `now`. Answers the id of a probe to write before
    /// it, unless a probe already precedes it: the probe after the last group, or the one
    /// before the open group.
    pub fn change(&mut self, now: Duration) -> Option<i32> {
        let probe = self.probes.newest().is_none().then(|| self.send_probe(now));
        let before = self.probes.newest().unwrap_or_default();
        self.changes.push_back(Bracket {
            before,
            after: None,
        });
        self.probe_last = false;
        probe
    }

    /// Answers the id of a probe to write after the changes written since the newest probe,
    /// if any were.
    pub fn close_changes(&mut self, now: Duration) -> Option<i32> {
        let open = self
            .changes
            .back()
            .is_some_and(|change| change.after.is_none());
        open.then(|| self.send_probe(now))
    }

    /// A packet that is not a predicted-state change or a probe went out.
    pub fn wrote(&mut self) {
        self.probe_last = false;
    }

    /// The server tick ends at `now`. Answers the id of a probe to write, unless the newest
    /// packet written is a probe.
    pub fn end_server_tick(&mut self, now: Duration) -> Option<i32> {
        self.server_ticks = self.server_ticks.saturating_add(1);
        if self.server_ticks >= LOAD_TICKS {
            self.loaded = true;
        }
        (!self.probe_last).then(|| self.send_probe(now))
    }

    /// A teleport with `id` went out; it is pending until an accept with this id.
    pub fn teleport_sent(&mut self, id: i32) {
        self.pending_teleport = Some(id);
    }

    /// The player spawns again; the client loads again.
    pub fn respawned(&mut self) {
        self.loaded = false;
        self.server_ticks = 0;
        self.silent_ticks = 0;
    }

    /// Whether a probe has waited for its answer for longer than 60 s at `now`.
    #[must_use]
    pub fn overdue(&self, now: Duration) -> bool {
        self.probes.waits_longer(now, MAX_ACK_DELAY)
    }

    /// The changes whose state the next client tick can still change, with their state for
    /// the last client tick.
    pub fn changes(&self) -> impl Iterator<Item = ChangeView> {
        (self.first_change..)
            .zip(&self.changes)
            .map(|(change, bracket)| ChangeView {
                change,
                state: self.state(*bracket),
            })
    }

    /// Takes one serverbound packet that layer 0 passed, which arrived at `now`, and adds the
    /// findings for the layer above to `reports`.
    pub fn serverbound(
        &mut self,
        checked: &Checked,
        now: Duration,
        reports: &mut Vec<Report>,
    ) -> Action {
        let client_tick = self.client_tick.saturating_add(1);
        let mut report = |finding| {
            reports.push(Report {
                finding,
                client_tick,
            });
        };
        for finding in checked.reports.iter() {
            report(finding);
        }
        match checked.packet {
            Serverbound::Move(player_move) => {
                let position = player_move.position.is_some();
                if position && self.positioned {
                    return Action::Disconnect(Disconnect::InvalidPlayerMovement);
                }
                if self.moved {
                    report(Finding::SecondMove);
                    return Action::Drop;
                }
                self.moved = true;
                self.positioned = position;
            }
            Serverbound::Accept(accept) => {
                if self.pending_teleport == Some(accept.teleport_id) {
                    self.pending_teleport = None;
                }
            }
            Serverbound::PlayerLoaded => self.loaded = true,
            Serverbound::Pong(id) => {
                match self.probes.answer(id) {
                    Answer::Oldest => {}
                    Answer::Skipped => report(Finding::SkippedAnswer),
                    Answer::Unknown => report(Finding::UnknownAnswer),
                }
                return Action::Drop;
            }
            Serverbound::ClientTickEnd => {
                if self.end_client_tick(now) {
                    report(Finding::TickRate);
                }
                if self.silent_ticks >= SILENT_LIMIT {
                    report(Finding::SilentTicks);
                }
                return Action::EndTick(client_tick);
            }
            Serverbound::VehicleMove(_) | Serverbound::Other => {}
        }
        Action::Forward
    }

    fn send_probe(&mut self, now: Duration) -> i32 {
        let id = self.probes.send(now);
        let probe = self.probes.newest().unwrap_or_default();
        for change in self.changes.iter_mut().rev() {
            if change.after.is_some() {
                break;
            }
            change.after = Some(probe);
        }
        self.balance
            .get_or_insert_with(|| now.saturating_sub(TICK_DRIFT));
        self.probe_last = true;
        id
    }

    fn state(&self, bracket: Bracket) -> ChangeState {
        if bracket
            .after
            .is_some_and(|after| after < self.answered_at_tick)
        {
            ChangeState::Applied
        } else if bracket.before < self.answered_at_tick {
            ChangeState::Window
        } else {
            ChangeState::NotApplied
        }
    }

    /// Ends the client tick; answers whether it breaks the tick rate.
    fn end_client_tick(&mut self, now: Duration) -> bool {
        self.client_tick = self.client_tick.saturating_add(1);
        let counted = self.loaded && self.pending_teleport.is_none();
        if self.positioned {
            self.silent_ticks = 0;
        } else if counted {
            self.silent_ticks = self.silent_ticks.saturating_add(1);
        }
        self.moved = false;
        self.positioned = false;

        while let Some(&front) = self.changes.front()
            && self.state(front) == ChangeState::Applied
        {
            self.changes.pop_front();
            self.first_change = self.first_change.saturating_add(1);
        }
        self.answered_at_tick = self.probes.answered();

        let Some(balance) = self.balance.as_mut() else {
            return false;
        };
        *balance = balance.saturating_add(CLIENT_TICK);
        let too_fast = *balance > now;
        if too_fast {
            *balance = balance.saturating_sub(CLIENT_TICK);
        }
        if let Some(answered) = self.probes.newest_answered_at() {
            *balance = (*balance).max(answered.saturating_sub(TICK_DRIFT));
        }
        too_fast
    }
}
