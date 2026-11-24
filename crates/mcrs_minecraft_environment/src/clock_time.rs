use mcrs_minecraft_core::codec::is_default;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockTime {
    pub total_ticks: i64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub partial_tick: f32,
    #[serde(default = "default_rate", skip_serializing_if = "is_one")]
    pub rate: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub paused: bool,
}

fn default_rate() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

impl Default for ClockTime {
    fn default() -> Self {
        ClockTime {
            total_ticks: 0,
            partial_tick: 0.0,
            rate: 1.0,
            paused: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockEdit {
    SetTotalTicks(i64),
    AddTicks(i32),
    SetPaused(bool),
    SetRate(f32),
}

impl ClockTime {
    pub fn apply(&mut self, edit: ClockEdit) {
        match edit {
            ClockEdit::SetTotalTicks(ticks) => {
                self.total_ticks = ticks;
                self.partial_tick = 0.0;
            }
            ClockEdit::AddTicks(ticks) => {
                self.total_ticks = self.total_ticks.saturating_add(i64::from(ticks)).max(0);
            }
            ClockEdit::SetPaused(paused) => self.paused = paused,
            ClockEdit::SetRate(rate) => self.rate = rate,
        }
    }

    pub fn advance(&mut self) {
        if self.paused {
            return;
        }
        let partial = self.partial_tick + self.rate;
        let whole = partial.floor();
        self.partial_tick = partial - whole;
        self.total_ticks = self.total_ticks.saturating_add(whole as i64);
    }

    pub fn advance_by_game_ticks(&mut self, game_ticks: i64) {
        let reached = f64::from(self.partial_tick) + game_ticks as f64 * f64::from(self.rate);
        let whole = reached.floor();
        self.partial_tick = (reached - whole) as f32;
        self.total_ticks = self.total_ticks.saturating_add(whole as i64);
    }
}
