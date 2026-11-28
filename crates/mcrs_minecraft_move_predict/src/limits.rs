#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub offset_threshold: f64,
    pub max_candidates: usize,
    pub replay_depth: usize,
    pub max_steps_per_tick: u32,
    pub max_open_windows: u64,
    pub max_pushers: usize,
    pub max_push_width: f64,
    pub instant_setback_offset: f64,
    pub buffer_limit: f64,
    pub buffer_cap: f64,
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
