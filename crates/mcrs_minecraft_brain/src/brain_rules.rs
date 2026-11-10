/// The rules of one brain that the stage and the host read outside of an evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrainRules {
    /// The largest horizontal distance from the mob at which an evaluation reads a player.
    pub player_range: f64,
    /// The idle time from which a reset of the idle time wakes the brain (GOAL-WAKE-005, event
    /// 5): the idle time at which the goals stop to start a stroll; `u32::MAX` for none.
    pub idle_limit: u32,
    /// A qualified player nearer than this resets the idle time of the mob (AIT-VAN-020).
    pub idle_reset_range: f64,
}
