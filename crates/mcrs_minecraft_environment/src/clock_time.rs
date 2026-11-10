use serde::{Deserialize, Serialize};

/// The state of one clock: whole ticks, the fraction of the next tick, the clock ticks one
/// game tick advances it by, and whether it stands still.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClockTime {
    pub total_ticks: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub partial_tick: f32,
    #[serde(skip_serializing_if = "is_one")]
    pub rate: f32,
    #[serde(skip_serializing_if = "is_false")]
    pub paused: bool,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

fn is_false(value: &bool) -> bool {
    !*value
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

/// A change that a command makes to a clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockEdit {
    /// The total ticks become the value, and the partial tick 0.
    SetTotalTicks(i64),
    /// The total ticks move by the value, but not below 0; the partial tick stays.
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

    /// One server tick that runs normally, for a clock that is not paused.
    pub fn advance(&mut self) {
        if self.paused {
            return;
        }
        let partial = self.partial_tick + self.rate;
        let whole = partial.floor();
        self.partial_tick = partial - whole;
        self.total_ticks = self.total_ticks.saturating_add(whole as i64);
    }

    /// The advance of a client clock by `game_ticks`, which can be 0 or negative. The client
    /// has no paused flag: a clock that stands still has the rate 0.
    pub fn advance_by_game_ticks(&mut self, game_ticks: i64) {
        let reached = f64::from(self.partial_tick) + game_ticks as f64 * f64::from(self.rate);
        let whole = reached.floor();
        self.partial_tick = (reached - whole) as f32;
        self.total_ticks = self.total_ticks.saturating_add(whole as i64);
    }
}
