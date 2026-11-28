use core::cmp::Ordering;
use std::collections::VecDeque;
use std::time::Duration;

use crate::{
    Acknowledged, Action, ChangeState, ChangeView, Checked, Disconnect, Finding, Report,
    Serverbound,
};

const TICK_DRIFT: Duration = Duration::from_millis(500);
const CLIENT_TICK: Duration = Duration::from_millis(50);
const SILENT_LIMIT: u32 = 20;
const LOAD_TICKS: u32 = 60;

#[derive(Clone, Debug, Default)]
pub struct MoveGuard {
    change_ticks: VecDeque<u64>,
    first_change: u64,
    acknowledged_at_client_tick: u64,
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

    pub fn change(&mut self, sent_tick: u64) {
        self.change_ticks.push_back(sent_tick);
    }

    pub fn end_server_tick(&mut self, now: Duration) {
        self.server_ticks = self.server_ticks.saturating_add(1);
        if self.server_ticks >= LOAD_TICKS {
            self.loaded = true;
        }
        self.balance
            .get_or_insert_with(|| now.saturating_sub(TICK_DRIFT));
    }

    pub fn teleport_sent(&mut self, id: i32) {
        self.pending_teleport = Some(id);
    }

    pub fn respawned(&mut self) {
        self.loaded = false;
        self.server_ticks = 0;
        self.silent_ticks = 0;
    }

    pub fn changes(&self) -> impl Iterator<Item = ChangeView> {
        (self.first_change..)
            .zip(&self.change_ticks)
            .map(|(change, &sent_tick)| ChangeView {
                change,
                state: self.state(sent_tick),
            })
    }

    pub fn serverbound(
        &mut self,
        checked: &Checked,
        now: Duration,
        acknowledged: Acknowledged,
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
            Serverbound::PlayerInput if !self.loaded => return Action::Drop,
            Serverbound::PlayerLoaded => self.loaded = true,
            Serverbound::Pong(_) => {
                report(Finding::UnknownAnswer);
                return Action::Drop;
            }
            Serverbound::ClientTickEnd => {
                if self.end_client_tick(now, acknowledged) {
                    report(Finding::TickRate);
                }
                if self.silent_ticks >= SILENT_LIMIT {
                    report(Finding::SilentTicks);
                }
                return Action::EndTick(client_tick);
            }
            Serverbound::VehicleMove(_) | Serverbound::PlayerInput | Serverbound::Other => {}
        }
        Action::Forward
    }

    fn state(&self, sent_tick: u64) -> ChangeState {
        change_state(sent_tick, self.acknowledged_at_client_tick)
    }

    fn end_client_tick(&mut self, now: Duration, acknowledged: Acknowledged) -> bool {
        self.client_tick = self.client_tick.saturating_add(1);
        let counted = self.loaded && self.pending_teleport.is_none();
        if self.positioned {
            self.silent_ticks = 0;
        } else if counted {
            self.silent_ticks = self.silent_ticks.saturating_add(1);
        }
        self.moved = false;
        self.positioned = false;

        while let Some(&front) = self.change_ticks.front()
            && self.state(front) == ChangeState::Applied
        {
            self.change_ticks.pop_front();
            self.first_change = self.first_change.saturating_add(1);
        }
        self.acknowledged_at_client_tick = acknowledged.tick;

        let Some(balance) = self.balance.as_mut() else {
            return false;
        };
        *balance = balance.saturating_add(CLIENT_TICK);
        let too_fast = *balance > now;
        if too_fast {
            *balance = balance.saturating_sub(CLIENT_TICK);
        }
        *balance = (*balance).max(acknowledged.sent_at.saturating_sub(TICK_DRIFT));
        too_fast
    }
}

fn change_state(sent_tick: u64, acknowledged_tick: u64) -> ChangeState {
    match acknowledged_tick.cmp(&sent_tick) {
        Ordering::Greater => ChangeState::Applied,
        Ordering::Equal => ChangeState::Window,
        Ordering::Less => ChangeState::NotApplied,
    }
}
