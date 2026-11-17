use crate::Finish;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dig {
    ticks: u32,
    progress: f32,
}

impl Dig {
    #[must_use]
    pub fn start(rate: f32) -> Self {
        Self {
            ticks: 0,
            progress: rate,
        }
    }

    pub fn advance(&mut self, rate: f32) {
        self.ticks = self.ticks.wrapping_add(1);
        self.progress += rate;
    }

    #[must_use]
    pub fn ticks(&self) -> u32 {
        self.ticks
    }

    #[must_use]
    pub fn progress(&self) -> f32 {
        self.progress
    }

    #[must_use]
    pub fn stage(&self) -> u8 {
        (self.progress * 10.0) as u32 as u8
    }

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
