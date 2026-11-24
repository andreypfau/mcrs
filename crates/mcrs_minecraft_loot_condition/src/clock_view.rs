use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_predicate::BlockHost;
use mcrs_minecraft_resource::Id;

pub trait ClockView {
    fn total_ticks(&self, clock: Id<WorldClock>) -> Option<i64>;
}

impl<F, E> ClockView for BlockHost<F, E> {
    fn total_ticks(&self, _: Id<WorldClock>) -> Option<i64> {
        None
    }
}
