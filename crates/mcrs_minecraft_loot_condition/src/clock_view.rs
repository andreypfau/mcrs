use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_resource::Id;

/// The world clocks of the world of an evaluation, as `time_check` reads them (LOOT-CND-015).
pub trait ClockView {
    /// The total ticks of `clock`. `None` for a clock that the world does not hold; the
    /// condition then reads 0 (CMD-TIME-005).
    fn total_ticks(&self, clock: Id<WorldClock>) -> Option<i64>;
}

impl ClockView for NoWorld {
    fn total_ticks(&self, _: Id<WorldClock>) -> Option<i64> {
        None
    }
}
