/// The configurable values of the prediction and of the response to a violation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// The largest offset of a match, in blocks.
    pub offset_threshold: f64,
    pub max_candidates: usize,
    /// The number of client ticks that a replay can go back.
    pub replay_depth: usize,
    /// The physics steps of one client tick, replays included.
    pub max_steps_per_tick: u32,
    pub max_open_windows: u64,
    pub max_pushers: usize,
    /// The widest push bound on one axis, in blocks.
    pub max_push_width: f64,
    pub instant_setback_offset: f64,
    pub buffer_limit: f64,
    pub buffer_cap: f64,
    /// The factor of the offset buffer in each client tick without a violation.
    pub buffer_decay: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            offset_threshold: 0.001,
            max_candidates: 64,
            replay_depth: 20,
            max_steps_per_tick: 64,
            max_open_windows: 4,
            max_pushers: 24,
            max_push_width: 0.1,
            instant_setback_offset: 0.1,
            buffer_limit: 1.0,
            buffer_cap: 4.0,
            buffer_decay: 0.999,
        }
    }
}
