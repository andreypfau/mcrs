use bevy_ecs::system::Local;

/// `run_if`-friendly cadence helper backed by a `Local<u32>` counter.
/// Returns a closure that yields `true` exactly once every `n` calls
/// (on the `n`-th call, then again on the `2n`-th, etc.). The first
/// call returns `false` unless `n == 1`.
///
/// `n == 0` is clamped to `1` ("every call") because the semantics of
/// "every zero calls" are undefined and naive `>= 0` arithmetic would
/// fire on every call without resetting the counter.
pub fn every_n_ticks(n: u32) -> impl FnMut(Local<u32>) -> bool {
    let n = n.max(1);
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
    use bevy_ecs::schedule::ScheduleLabel;

    #[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct DummySchedule;

    #[test]
    fn every_n_ticks_zero_clamps_to_every_call() {
        // Drive the helper through Bevy's schedule so the Local<u32>
        // counter persists across calls. With n clamped to 1, every
        // invocation should fire.
        use bevy_ecs::prelude::*;

        #[derive(Resource, Default)]
        struct FireLog(Vec<bool>);

        let mut helper = every_n_ticks(0);
        let mut app = App::new();
        app.init_resource::<FireLog>();
        app.add_schedule(Schedule::new(DummySchedule));
        app.add_systems(
            DummySchedule,
            (move |local: Local<u32>, mut log: ResMut<FireLog>| {
                let fired = helper(local);
                log.0.push(fired);
            },)
                .into_configs(),
        );
        for _ in 0..5 {
            app.world_mut().run_schedule(DummySchedule);
        }
        let log = &app.world().resource::<FireLog>().0;
        assert_eq!(log.len(), 5);
        for (i, fired) in log.iter().enumerate() {
            assert!(*fired, "tick {} should fire when n clamps to 1", i + 1);
        }
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
