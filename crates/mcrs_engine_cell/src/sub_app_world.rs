use bevy_app::{
    First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last, PostStartup,
    PostUpdate, PreStartup, PreUpdate, Startup, SubApp, Update,
};
use bevy_ecs::schedule::{InternedScheduleLabel, ScheduleLabel};
use bevy_ecs::world::World;

use crate::cell_world::CellWorld;
use crate::tick::Tick;

/// A Bevy [`SubApp`] as a [`CellWorld`].
///
/// A tick runs the startup schedules on the first tick only, then `apply`, then `First`,
/// `PreUpdate`, the five fixed schedules, `Update`, `PostUpdate` and `Last`, each exactly once,
/// then `take`. The fixed schedules do not follow `Time<Fixed>`: one tick is one fixed step.
/// A schedule that the sub-app lacks is skipped.
pub struct SubAppWorld<I, O> {
    sub_app: SubApp,
    apply: fn(&mut World, Tick, I),
    take: fn(&mut World) -> O,
    phase: Phase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Starting,
    Running,
}

impl<I, O> SubAppWorld<I, O> {
    /// Wraps `sub_app`. `apply` writes the input of a tick into the world, and `take` reads the
    /// output out of it.
    #[must_use]
    pub fn new(sub_app: SubApp, apply: fn(&mut World, Tick, I), take: fn(&mut World) -> O) -> Self {
        Self {
            sub_app,
            apply,
            take,
            phase: Phase::Starting,
        }
    }

    pub fn sub_app_mut(&mut self) -> &mut SubApp {
        &mut self.sub_app
    }
}

impl<I: Send + 'static, O: Send + 'static> CellWorld for SubAppWorld<I, O> {
    type Input = I;
    type Output = O;

    fn tick(&mut self, tick: Tick, input: I) -> O {
        let world = self.sub_app.world_mut();
        if self.phase == Phase::Starting {
            run(
                world,
                [PreStartup.intern(), Startup.intern(), PostStartup.intern()],
            );
            self.phase = Phase::Running;
        }
        (self.apply)(world, tick, input);
        run(
            world,
            [
                First.intern(),
                PreUpdate.intern(),
                FixedFirst.intern(),
                FixedPreUpdate.intern(),
                FixedUpdate.intern(),
                FixedPostUpdate.intern(),
                FixedLast.intern(),
                Update.intern(),
                PostUpdate.intern(),
                Last.intern(),
            ],
        );
        let output = (self.take)(world);
        world.clear_trackers();
        output
    }
}

fn run<const N: usize>(world: &mut World, labels: [InternedScheduleLabel; N]) {
    for label in labels {
        // Fails only for a schedule the sub-app lacks, which has no systems to run.
        let _ = world.try_run_schedule(label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bevy_ecs::resource::Resource;
    use bevy_ecs::system::ResMut;
    use bevy_tasks::block_on;
    use mcrs_engine_link::Placement;

    use crate::cell_output::CellOutput;
    use crate::cells::Cells;

    #[derive(Resource, Default)]
    struct Log(Vec<u64>);

    fn started(mut log: ResMut<Log>) {
        log.0.push(0);
    }

    fn fixed(mut log: ResMut<Log>) {
        if let Some(last) = log.0.last_mut() {
            *last += 1;
        }
    }

    fn apply(world: &mut World, tick: Tick, input: u64) {
        world.resource_mut::<Log>().0.push(tick.0 * 100 + input);
    }

    fn take(world: &mut World) -> Vec<u64> {
        world.resource::<Log>().0.clone()
    }

    #[test]
    fn a_sub_app_starts_once_and_runs_its_schedules_on_every_tick() {
        let mut sub_app = SubApp::new();
        sub_app.init_resource::<Log>();
        sub_app.add_systems(Startup, started);
        sub_app.add_systems(FixedUpdate, fixed);
        let mut cells = Cells::new();
        let cell = cells.add(SubAppWorld::new(sub_app, apply, take), Placement::Thread);

        let mut outputs = Vec::new();
        for tick in 1..=2 {
            outputs.extend(block_on(cells.tick(Tick(tick), |_| 7)).expect("no world is lent"));
        }

        assert_eq!(
            outputs,
            [
                CellOutput {
                    cell,
                    output: vec![0, 108]
                },
                CellOutput {
                    cell,
                    output: vec![0, 108, 208]
                },
            ]
        );
    }
}
