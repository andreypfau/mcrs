//! Cross-section convergence driver and sub-schedule label.
//!
//! `light_converge_driver` is the workspace's only sanctioned intra-tick
//! `for`/`loop` per the concurrency conventions exception "bounded intra-tick
//! convergence loop". It is gated on `MAX_ITERATIONS` iterations, `HARD_BUDGET`
//! wall time, and absence of dirty sections. The driver body is a placeholder
//! at this point; this module currently ships only the type surface so the
//! distribute and convergence wiring can be developed in parallel.
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use std::time::{Duration, Instant};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LightConvergeSchedule;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LightConvergeSet {
    PropagateDecrease,
    DistributeDecrease,
    PropagateIncrease,
    DistributeIncrease,
}

pub const MAX_ITERATIONS: usize = 32;
pub const HARD_BUDGET: Duration = Duration::from_millis(25);
pub const SOFT_BUDGET: Duration = Duration::from_millis(10);
pub const PENDING_EGRESS_CAP: usize = 256;

#[derive(Resource, Copy, Clone)]
pub struct TickStart(pub Instant);

impl Default for TickStart {
    fn default() -> Self {
        Self(Instant::now())
    }
}

pub fn set_tick_start(mut tick_start: ResMut<TickStart>) {
    tick_start.0 = Instant::now();
}

pub fn light_converge_driver(world: &mut World) {
    let _ = world;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consts_have_expected_values() {
        assert_eq!(MAX_ITERATIONS, 32);
        assert_eq!(HARD_BUDGET, Duration::from_millis(25));
        assert_eq!(SOFT_BUDGET, Duration::from_millis(10));
        assert_eq!(PENDING_EGRESS_CAP, 256);
    }

    #[test]
    fn tick_start_default_uses_instant_now() {
        let before = Instant::now();
        let t = TickStart::default();
        let after = Instant::now();
        assert!(t.0 >= before);
        assert!(t.0 <= after);
    }

    #[test]
    fn light_converge_set_variants_compile() {
        let _ = LightConvergeSet::PropagateDecrease;
        let _ = LightConvergeSet::DistributeDecrease;
        let _ = LightConvergeSet::PropagateIncrease;
        let _ = LightConvergeSet::DistributeIncrease;
    }
}
