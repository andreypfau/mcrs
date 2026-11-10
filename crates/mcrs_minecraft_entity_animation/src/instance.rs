use crate::clip::{Clip, LoopMode, TimeUpdate};

/// The phase of a clip instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Phase {
    /// The entry of the play list has the weight 0, or it never played.
    #[default]
    Stopped,
    /// The start delay runs: the time stays 0 and the clip adds nothing.
    StartDelay,
    Playing,
    /// The loop delay runs: the clip adds its pose at the time 0.
    LoopDelay,
}

/// The state of one clip instance of one entity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Instance {
    pub phase: Phase,
    /// `query.anim_time`, in seconds.
    pub time: f32,
    /// The weight in the play list of the current frame; 0 when the instance adds nothing.
    pub weight: f32,
    /// The entity clock, in ticks, at which the time was 0.
    pub(crate) start_clock: f32,
    /// The seconds that remain of a start delay or a loop delay.
    pub(crate) delay: f32,
    /// The next update is the first of the instance: `query.delta_time` is 0 in it.
    pub(crate) first_update: bool,
}

/// The time that the next update of an instance computes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Step {
    /// `query.delta_time` of the update.
    pub delta_time: f32,
    /// The entity clock of the frame, in ticks.
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

    /// Runs down a delay. Returns whether the instance updates its time in this frame.
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

    /// `query.delta_time` for the time update of this frame.
    pub(crate) fn delta_time(&self, step: Step) -> f32 {
        if self.first_update {
            0.0
        } else {
            step.delta_time
        }
    }

    /// The time of the clock update in this frame.
    pub(crate) fn clock_time(&self, step: Step) -> f32 {
        if self.first_update {
            0.0
        } else {
            (step.clock - self.start_clock) / 20.0
        }
    }

    /// Stores the updated time and wraps it. Returns whether the time wrapped, so that the
    /// caller evaluates the loop delay.
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

    /// The time at which the channels are sampled; `None` when the instance adds nothing.
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
