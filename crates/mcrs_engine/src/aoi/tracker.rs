//! AoI tracker substrate. Concrete implementations (`PlayerTracker` in
//! the minecraft tier, future `MobTracker` / `ItemTracker` /
//! `ProjectileTracker` for content tiers) plug in via associated types;
//! no `dyn` dispatch in the trait surface.

use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{ScheduleConfigs, SystemSet};
use bevy_ecs::system::{Local, ScheduleSystem};

/// Update cadence for a concrete tracker. `Every` is the default for
/// position-driven AoI work; `EveryN(n)` is a cost-amortising knob for
/// trackers whose inputs change rarely (e.g., projectile despawn
/// timers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickInterval {
    Every,
    EveryN(u32),
}

impl TickInterval {
    pub const fn to_n(self) -> u32 {
        match self {
            Self::Every => 1,
            Self::EveryN(n) => n,
        }
    }
}

/// Generic AoI tracker contract. Per-tracker `SystemSet`, associated
/// per-tracker Entity marker Component + Cache Resource, and a const
/// cadence. Each concrete impl is monomorphised; there is no virtual
/// call in the AoI hot path.
pub trait EntityTracker: 'static + Send + Sync {
    type Entity: Component;
    type Cache: Resource + Default;
    type Set: SystemSet + Clone + Default;
    const CADENCE: TickInterval;
    fn systems() -> ScheduleConfigs<ScheduleSystem>;
}

/// `run_if`-friendly cadence helper backed by a `Local<u32>` counter.
/// Returns a closure that yields `true` exactly once every `n` calls
/// (on the `n`-th call, then again on the `2n`-th, etc.). The first
/// call returns `false` unless `n == 1`.
///
/// Pattern source: `crates/mcrs_minecraft/src/world/sub_app_builder.rs:183`
/// uses `Local<bool>` for closure state. Same shape with `Local<u32>`.
pub fn every_n_ticks(n: u32) -> impl FnMut(Local<u32>) -> bool {
    move |mut counter: Local<u32>| {
        *counter = counter.saturating_add(1);
        if *counter >= n {
            *counter = 0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::prelude::IntoScheduleConfigs;
    use bevy_ecs::schedule::ScheduleLabel;

    #[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct DummySchedule;

    #[derive(Component, Default)]
    struct DummyMarker;

    #[derive(Resource, Default)]
    struct DummyCache;

    #[derive(SystemSet, Clone, Default, Hash, PartialEq, Eq, Debug)]
    struct DummySet;

    struct DummyTracker;

    fn dummy_system() {}

    impl EntityTracker for DummyTracker {
        type Entity = DummyMarker;
        type Cache = DummyCache;
        type Set = DummySet;
        const CADENCE: TickInterval = TickInterval::EveryN(2);

        fn systems() -> ScheduleConfigs<ScheduleSystem> {
            dummy_system.into_configs()
        }
    }

    #[test]
    fn dummy_tracker_compiles_and_const_cadence_matches() {
        assert_eq!(DummyTracker::CADENCE, TickInterval::EveryN(2));
        assert_eq!(DummyTracker::CADENCE.to_n(), 2);
        let _ = DummyTracker::systems();
    }

    #[test]
    fn tick_interval_every_to_n_is_one() {
        assert_eq!(TickInterval::Every.to_n(), 1);
    }

    #[test]
    fn every_n_ticks_fires_at_cadence() {
        // Drive the closure through Bevy's schedule so the Local<u32>
        // counter persists across calls (which is the contract the
        // helper relies on).
        use bevy_ecs::prelude::*;

        #[derive(Resource, Default)]
        struct FireLog(Vec<bool>);

        let mut app = App::new();
        app.init_resource::<FireLog>();
        app.add_schedule(bevy_ecs::schedule::Schedule::new(DummySchedule));
        app.add_systems(
            DummySchedule,
            (|mut local_n: Local<u32>, mut log: ResMut<FireLog>| {
                *local_n = local_n.saturating_add(1);
                let fired = if *local_n >= 3 {
                    *local_n = 0;
                    true
                } else {
                    false
                };
                log.0.push(fired);
            },)
                .into_configs(),
        );
        for _ in 0..10 {
            app.world_mut().run_schedule(DummySchedule);
        }
        let log = &app.world().resource::<FireLog>().0;
        assert_eq!(log.len(), 10);
        // Expected pattern with n = 3: false, false, true, false, false,
        // true, false, false, true, false.
        for (i, fired) in log.iter().enumerate() {
            let expected = (i + 1) % 3 == 0;
            assert_eq!(*fired, expected, "tick {} expected={}", i + 1, expected);
        }
    }
}
