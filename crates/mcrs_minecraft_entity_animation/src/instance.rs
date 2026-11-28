use crate::clip::{Clip, LoopMode, TimeUpdate};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Phase {
    #[default]
    Stopped,
    StartDelay,
    Playing,
    LoopDelay,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Instance {
    pub phase: Phase,
    pub time: f32,
    pub weight: f32,
    pub(crate) start_clock: f32,
    pub(crate) delay: f32,
    pub(crate) first_update: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Step {
    pub delta_time: f32,
    pub clock: f32,
}

impl Instance {
    pub(crate) fn start(&mut self, clock: f32, start_delay: f32) {
        *self = Self {
            phase: if start_delay > 0.0 {
                Phase::StartDelay
            } else {
                Phase::Playing
            },
            delay: start_delay,
            start_clock: clock,
            first_update: true,
            ..Self::default()
        };
    }

    pub(crate) fn run_delay(&mut self, step: Step) -> bool {
        match self.phase {
            Phase::Stopped => false,
            Phase::Playing => true,
            Phase::StartDelay | Phase::LoopDelay => {
                self.delay -= step.delta_time;
                if self.delay > 0.0 {
                    return false;
                }
                self.phase = Phase::Playing;
                self.start_clock = step.clock;
                self.time = 0.0;
                self.first_update = true;
                true
            }
        }
    }

    pub(crate) fn delta_time(&self, step: Step) -> f32 {
        if self.first_update {
            0.0
        } else {
            step.delta_time
        }
    }

    pub(crate) fn clock_time(&self, step: Step) -> f32 {
        if self.first_update {
            0.0
        } else {
            (step.clock - self.start_clock) / 20.0
        }
    }

    pub(crate) fn set_time(&mut self, clip: &Clip, time: f32, clock: f32) -> bool {
        self.first_update = false;
        if !time.is_finite() {
            return false;
        }
        self.time = time;
        let length = clip.length;
        if clip.loop_mode != LoopMode::Loop || length <= 0.0 || (0.0..length).contains(&time) {
            return false;
        }
        self.time = time.rem_euclid(length);
        if clip.time_update == TimeUpdate::Clock {
            self.start_clock = clock - self.time * 20.0;
        }
        true
    }

    pub(crate) fn begin_loop_delay(&mut self, delay: f32) {
        if delay > 0.0 {
            self.phase = Phase::LoopDelay;
            self.delay = delay;
            self.time = 0.0;
        }
    }

    #[must_use]
    pub fn sample_time(&self, clip: &Clip) -> Option<f32> {
        match self.phase {
            Phase::Stopped | Phase::StartDelay => None,
            Phase::LoopDelay => Some(0.0),
            Phase::Playing => {
                let length = clip.length;
                match clip.loop_mode {
                    _ if length <= 0.0 => Some(self.time),
                    LoopMode::Loop => Some(self.time),
                    LoopMode::HoldOnLastFrame => Some(self.time.min(length)),
                    LoopMode::Once => (self.time < length).then_some(self.time),
                }
            }
        }
    }
}
