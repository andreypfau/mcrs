use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_resource::Id;

pub trait ClockView {
    fn total_ticks(&self, clock: Id<WorldClock>) -> Option<i64>;
}

impl ClockView for NoWorld {
    fn total_ticks(&self, _: Id<WorldClock>) -> Option<i64> {
        None
    }
}
