use mcrs_minecraft_predicate::NoWorld;

use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_resource::Id;

pub trait ClockView {
    fn total_ticks(&self, _clock: Id<WorldClock>) -> Option<i64> {
        None
    }
}

impl ClockView for NoWorld {}
