use crate::Finish;

/// The tick count and the progress of one dig.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dig {
    ticks: u32,
    progress: f32,
}

impl Dig {
    /// The dig of a start action: the tick count 0, and the rate of that tick as the progress.
    #[must_use]
    pub fn start(rate: f32) -> Self {
        Self {
            ticks: 0,
            progress: rate,
        }
    }

    /// Adds one tick with the rate of its own inputs.
    pub fn advance(&mut self, rate: f32) {
        // Wraps and does not saturate: a reader takes the tick count modulo 4.
        self.ticks = self.ticks.wrapping_add(1);
        self.progress += rate;
    }

    /// The number of ticks after the start action.
    #[must_use]
    pub fn ticks(&self) -> u32 {
        self.ticks
    }

    /// The sum of the rates from the start action to the last tick.
    #[must_use]
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// `floor(progress × 10)` modulo 256. It has no limit of 9.
    #[must_use]
    pub fn stage(&self) -> u8 {
        (self.progress * 10.0) as u32 as u8
    }

    /// What a finish action does. It reads the progress of the last tick and adds no rate; `rate`
    /// is the rate that the next tick would add.
    #[must_use]
    pub fn finish(&self, rate: f32) -> Finish {
        if self.progress >= 0.7 {
            Finish::Break
        } else if rate > 0.0 {
            Finish::DelayedBreak
        } else {
            Finish::Stay
        }
    }
}
