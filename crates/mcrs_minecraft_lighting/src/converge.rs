//! Cross-section convergence driver and sub-schedule label.
//!
//! `light_converge_driver` is the workspace's only sanctioned intra-tick
//! `for`/`loop` per the concurrency conventions exception for the bounded
//! intra-tick convergence loop. It is gated on `MAX_ITERATIONS` iterations,
//! `HARD_BUDGET` wall time, and absence of dirty sections.
//!
//! The driver runs `LightConvergeSchedule` against the host `World` and
//! polls `Query<(), With<LightDirty>>` after each iteration. Quiescence
//! (zero dirty matches) is the primary termination condition; the hard
//! wall-clock budget and the max-iteration cap are the safety nets.
//! Cap-fire emits a `tracing::warn!` and increments `LIGHT_CONVERGE_CAPPED_TOTAL`;
//! every termination path increments `LIGHT_CONVERGE_ITERATIONS_TOTAL` by
//! the number of iterations consumed this tick.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::components::LightDirty;
use crate::telemetry::{LIGHT_CONVERGE_CAPPED_TOTAL, LIGHT_CONVERGE_ITERATIONS_TOTAL};

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

pub fn light_converge_driver(world: &mut World) {
    // Budget the cascade against the driver's own start, not against the
    // global tick start. Upstream systems (worldgen, chunk reconcile,
    // block updates) routinely burn 30-100 ms before the lighting stage
    // even runs; sharing the deadline with them meant the driver would
    // cap-on-iteration-1 every tick during chunk load, regardless of how
    // little lighting work was actually pending. Anchoring the budget
    // here gives the driver a predictable per-tick wall-clock slice
    // (`HARD_BUDGET`) and keeps the cap signal meaningful — it fires only
    // when the cascade itself is starved, not because the tick was busy.
    let driver_start = Instant::now();
    #[cfg(feature = "lighting-trace")]
    let _span = tracing::info_span!("light_converge_driver", iter = tracing::field::Empty).entered();

    for iteration in 0..MAX_ITERATIONS {
        world.run_schedule(LightConvergeSchedule);

        let dirty_count = world
            .query_filtered::<(), With<LightDirty>>()
            .iter(world)
            .count();

        if dirty_count == 0 {
            #[cfg(feature = "lighting-trace")]
            _span.record("iter", iteration + 1);
            LIGHT_CONVERGE_ITERATIONS_TOTAL
                .fetch_add(iteration as u64 + 1, Ordering::Relaxed);
            return;
        }

        let elapsed = Instant::now().duration_since(driver_start);
        if elapsed >= HARD_BUDGET {
            #[cfg(feature = "lighting-trace")]
            _span.record("iter", iteration + 1);
            LIGHT_CONVERGE_ITERATIONS_TOTAL
                .fetch_add(iteration as u64 + 1, Ordering::Relaxed);
            LIGHT_CONVERGE_CAPPED_TOTAL.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                iteration = iteration + 1,
                elapsed_ms = elapsed.as_millis() as u64,
                "light converge hit HARD_BUDGET cap"
            );
            return;
        }
        if elapsed >= SOFT_BUDGET {
            tracing::warn!(
                iteration = iteration + 1,
                elapsed_ms = elapsed.as_millis() as u64,
                "light converge near SOFT_BUDGET"
            );
        }
    }

    #[cfg(feature = "lighting-trace")]
    _span.record("iter", MAX_ITERATIONS);
    LIGHT_CONVERGE_ITERATIONS_TOTAL.fetch_add(MAX_ITERATIONS as u64, Ordering::Relaxed);
    LIGHT_CONVERGE_CAPPED_TOTAL.fetch_add(1, Ordering::Relaxed);
    tracing::warn!(iteration = MAX_ITERATIONS, "light converge hit MAX_ITERATIONS cap");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::TELEMETRY_TEST_LOCK;
    use bevy_ecs::schedule::Schedule;

    #[test]
    fn consts_have_expected_values() {
        assert_eq!(MAX_ITERATIONS, 32);
        assert_eq!(HARD_BUDGET, Duration::from_millis(25));
        assert_eq!(SOFT_BUDGET, Duration::from_millis(10));
        assert_eq!(PENDING_EGRESS_CAP, 256);
    }

    #[test]
    fn light_converge_set_variants_compile() {
        let _ = LightConvergeSet::PropagateDecrease;
        let _ = LightConvergeSet::DistributeDecrease;
        let _ = LightConvergeSet::PropagateIncrease;
        let _ = LightConvergeSet::DistributeIncrease;
    }

    /// Helper: clear-`LightDirty` system used as a stub schedule body in
    /// the quiescence test.
    fn clear_one_dirty_section(mut commands: Commands, dirty: Query<Entity, With<LightDirty>>) {
        for e in dirty.iter() {
            commands.entity(e).remove::<LightDirty>();
        }
    }

    /// Helper: re-mark every section dirty (idempotent under With<LightDirty>
    /// filter — the section will be queried again next iteration because
    /// `Commands::insert` re-applies the marker). Forces the convergence
    /// driver to never reach quiescence.
    fn re_insert_dirty(mut commands: Commands, dirty: Query<Entity, With<LightDirty>>) {
        for e in dirty.iter() {
            commands.entity(e).insert(LightDirty);
        }
    }

    fn build_driver_app_with_schedule<F>(schedule_builder: F) -> App
    where
        F: FnOnce() -> Schedule,
    {
        let mut app = App::new();
        app.world_mut().add_schedule(schedule_builder());
        app
    }

    #[test]
    fn light_converge_driver_terminates_on_quiescence() {
        let _lock = TELEMETRY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut app = build_driver_app_with_schedule(|| {
            let mut schedule = Schedule::new(LightConvergeSchedule);
            schedule.add_systems(clear_one_dirty_section);
            schedule
        });

        let _section = app.world_mut().spawn(LightDirty).id();

        let before = crate::telemetry::snapshot();
        light_converge_driver(app.world_mut());
        let after = crate::telemetry::snapshot();

        assert_eq!(
            after.iterations - before.iterations,
            1,
            "quiescence at iteration 0 records iterations += 1"
        );
        assert_eq!(
            after.capped - before.capped,
            0,
            "quiescence does not increment capped counter"
        );
    }

    #[test]
    fn light_converge_driver_terminates_on_max_iterations() {
        let _lock = TELEMETRY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // Stub schedule re-inserts LightDirty every iteration so the driver
        // never reaches quiescence; each iteration is sub-millisecond so the
        // 25 ms `HARD_BUDGET` is well out of reach. The driver must exit via
        // the `MAX_ITERATIONS` ceiling.
        let mut app = build_driver_app_with_schedule(|| {
            let mut schedule = Schedule::new(LightConvergeSchedule);
            schedule.add_systems(re_insert_dirty);
            schedule
        });

        let _section = app.world_mut().spawn(LightDirty).id();

        let before = crate::telemetry::snapshot();
        light_converge_driver(app.world_mut());
        let after = crate::telemetry::snapshot();

        assert_eq!(
            after.iterations - before.iterations,
            MAX_ITERATIONS as u64,
            "MAX_ITERATIONS cap records iterations += MAX_ITERATIONS"
        );
        assert_eq!(
            after.capped - before.capped,
            1,
            "MAX_ITERATIONS cap increments capped counter once"
        );
    }

    /// Stub schedule body that sleeps long enough for a single iteration to
    /// blow the `HARD_BUDGET` wall-clock budget, and re-marks the section
    /// dirty so the driver doesn't exit via the quiescence path first.
    fn slow_redirty_30ms(
        mut commands: Commands,
        dirty: Query<Entity, With<LightDirty>>,
    ) {
        std::thread::sleep(Duration::from_millis(30));
        for e in dirty.iter() {
            commands.entity(e).insert(LightDirty);
        }
    }

    #[test]
    fn light_converge_driver_terminates_on_hard_budget() {
        let _lock = TELEMETRY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut app = build_driver_app_with_schedule(|| {
            let mut schedule = Schedule::new(LightConvergeSchedule);
            schedule.add_systems(slow_redirty_30ms);
            schedule
        });

        let _section = app.world_mut().spawn(LightDirty).id();

        let before = crate::telemetry::snapshot();
        light_converge_driver(app.world_mut());
        let after = crate::telemetry::snapshot();

        // The schedule body sleeps 30 ms before re-inserting LightDirty.
        // After the first run_schedule the driver observes elapsed >=
        // HARD_BUDGET (25 ms) and exits via the cap path.
        assert_eq!(
            after.iterations - before.iterations,
            1,
            "HARD_BUDGET fires after iteration 1; records iterations += 1"
        );
        assert_eq!(
            after.capped - before.capped,
            1,
            "HARD_BUDGET cap increments capped counter once"
        );
    }

    /// Cross-check that the iterations + capped counters move independently
    /// across the three termination paths.
    #[test]
    fn driver_records_iterations_and_capped() {
        let _lock = TELEMETRY_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        // Quiescence path: iterations += 1, capped unchanged.
        let mut app1 = build_driver_app_with_schedule(|| {
            let mut s = Schedule::new(LightConvergeSchedule);
            s.add_systems(clear_one_dirty_section);
            s
        });
        let _ = app1.world_mut().spawn(LightDirty).id();
        let b1 = crate::telemetry::snapshot();
        light_converge_driver(app1.world_mut());
        let a1 = crate::telemetry::snapshot();
        assert_eq!(a1.iterations - b1.iterations, 1);
        assert_eq!(a1.capped - b1.capped, 0);

        // MAX_ITERATIONS path: iterations += MAX_ITERATIONS, capped += 1.
        let mut app2 = build_driver_app_with_schedule(|| {
            let mut s = Schedule::new(LightConvergeSchedule);
            s.add_systems(re_insert_dirty);
            s
        });
        let _ = app2.world_mut().spawn(LightDirty).id();
        let b2 = crate::telemetry::snapshot();
        light_converge_driver(app2.world_mut());
        let a2 = crate::telemetry::snapshot();
        assert_eq!(a2.iterations - b2.iterations, MAX_ITERATIONS as u64);
        assert_eq!(a2.capped - b2.capped, 1);
    }
}
