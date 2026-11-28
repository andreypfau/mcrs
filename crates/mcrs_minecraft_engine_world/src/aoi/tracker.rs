use bevy_ecs::system::Local;

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
    fn every_n_ticks_fires_at_cadence() {
        use bevy_ecs::prelude::*;

        #[derive(Resource, Default)]
        struct FireLog(Vec<bool>);

        let mut helper = every_n_ticks(3);
        let mut app = App::new();
        app.init_resource::<FireLog>();
        app.add_schedule(bevy_ecs::schedule::Schedule::new(DummySchedule));
        app.add_systems(
            DummySchedule,
            (move |local: Local<u32>, mut log: ResMut<FireLog>| {
                let fired = helper(local);
                log.0.push(fired);
            },)
                .into_configs(),
        );
        for _ in 0..10 {
            app.world_mut().run_schedule(DummySchedule);
        }
        let log = &app.world().resource::<FireLog>().0;
        assert_eq!(log.len(), 10);
        for (i, fired) in log.iter().enumerate() {
            let expected = (i + 1) % 3 == 0;
            assert_eq!(*fired, expected, "tick {} expected={}", i + 1, expected);
        }
    }
}
