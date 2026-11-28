use bevy_app::prelude::*;
use bevy_app::{App, Plugin, TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use bevy_asset::AssetPlugin;
use bevy_ecs::schedule::{Schedules, SingleThreadedExecutor};
use bevy_ecs::world::World;
use bevy_time::{Fixed, Time, TimePlugin, TimeUpdateStrategy};
use mcrs_engine_link::Placement;
use std::num::{NonZeroU32, NonZeroUsize};
use std::time::{Duration, Instant};

use crate::tick_pace::TickPace;
use crate::world_placement::WorldPlacement;

pub struct VoxelServerPlugin {
    pub tick_rate: NonZeroU32,
    pub owns_task_pools: bool,
    pub asset_path: Option<String>,
    pub worlds: Placement,
    pub compute_threads: Option<NonZeroUsize>,
}

impl Plugin for VoxelServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldPlacement(self.worlds));
        let compute_threads = match (self.compute_threads, self.worlds) {
            _ if !self.owns_task_pools => None,
            (Some(threads), _) => Some(threads.get()),
            (None, Placement::Inline) => Some(1),
            (None, _) => std::thread::available_parallelism()
                .ok()
                .map(NonZeroUsize::get),
        };
        let task_pool_options = match compute_threads {
            Some(threads) => TaskPoolOptions {
                compute: TaskPoolThreadAssignmentPolicy {
                    min_threads: threads,
                    max_threads: threads,
                    ..TaskPoolOptions::default().compute
                },
                ..TaskPoolOptions::default()
            },
            None => TaskPoolOptions::default(),
        };
        app.add_plugins(TaskPoolPlugin { task_pool_options });

        app.edit_schedule(Update, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        });

        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        app.insert_resource(Time::<Fixed>::from_hz(self.tick_rate.get() as f64));
        app.insert_resource(TickPace::new(self.tick_rate.get()));
        app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        let mut assets = AssetPlugin {
            watch_for_changes_override: Some(false),
            ..AssetPlugin::default()
        };
        if let Some(path) = &self.asset_path {
            assets.file_path = path.clone();
        }
        app.add_plugins(assets);
    }

    fn cleanup(&self, app: &mut App) {
        if app.world().get_resource::<WorldPlacement>() == Some(&WorldPlacement(Placement::Inline))
        {
            run_schedules_on_one_thread(app.world_mut());
        }
    }
}

const MAX_BACKLOG: Duration = Duration::from_secs(2);

fn next_deadline(previous: Instant, tick: Duration, at_once: bool, now: Instant) -> Instant {
    if at_once {
        return now;
    }
    let Some(next) = previous.checked_add(tick) else {
        return now;
    };
    if now.saturating_duration_since(next) > MAX_BACKLOG {
        tracing::warn!(
            behind_ms = now.saturating_duration_since(next).as_millis() as u64,
            "the server cannot keep up; it skips the ticks it is behind"
        );
        return now;
    }
    next
}

pub fn run_schedules_on_one_thread(world: &mut World) {
    let Some(mut schedules) = world.get_resource_mut::<Schedules>() else {
        return;
    };
    for (_, schedule) in schedules.iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
}

struct SendableApp(App);

// SAFETY: `App` is `!Send` only because of its boxed runner closure. `spawn_server_thread`
// replaces the runner with one that captures nothing before it wraps the `App`.
unsafe impl Send for SendableApp {}

impl SendableApp {
    fn into_inner(self) -> App {
        self.0
    }
}

pub fn spawn_server_thread(
    app: App,
    run: impl FnOnce(App) + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    let mut app = app;
    app.set_runner(|_| AppExit::Success);
    let app = SendableApp(app);
    std::thread::Builder::new()
        .name("voxel-server".to_string())
        .spawn(move || run(app.into_inner()))
}

pub fn run_server_loop(
    mut app: App,
    tick_rate: NonZeroU32,
    mut between_ticks: impl FnMut(&mut App),
    mut until: impl FnMut(&mut App, Instant),
) -> AppExit {
    let tick = Duration::from_secs_f64(1.0 / tick_rate.get() as f64);
    app.finish();
    app.cleanup();
    if let Some(exit) = app.should_exit() {
        return exit;
    }
    let mut next = Instant::now();
    loop {
        let start = Instant::now();
        if let Some(mut pace) = app.world_mut().get_resource_mut::<TickPace>() {
            pace.begin_tick();
        }
        app.update();
        between_ticks(&mut app);
        let elapsed = start.elapsed();
        tracing::trace!(target: "mcrs_tick", micros = elapsed.as_micros() as u64);
        if let Some(exit) = app.should_exit() {
            return exit;
        }
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        let (at_once, target) =
            app.world_mut()
                .get_resource_mut::<TickPace>()
                .map_or((false, tick), |mut pace| {
                    (
                        pace.end_tick(nanos),
                        Duration::from_nanos(pace.target_nanos()),
                    )
                });
        next = next_deadline(next, target, at_once, Instant::now());
        until(&mut app, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: Duration = Duration::from_millis(50);

    #[test]
    fn a_late_wake_does_not_move_the_ticks_after_it() {
        let start = Instant::now();
        let first = next_deadline(start, TICK, false, start + Duration::from_millis(57));
        assert_eq!(first, start + TICK);
        let second = next_deadline(first, TICK, false, first + Duration::from_millis(7));
        assert_eq!(
            second,
            start + 2 * TICK,
            "the schedule stays on 50 ms steps"
        );
    }

    #[test]
    fn a_backlog_up_to_two_seconds_is_caught_up_and_a_longer_one_is_dropped() {
        let start = Instant::now();
        let late = start + Duration::from_millis(1500);
        assert_eq!(next_deadline(start, TICK, false, late), start + TICK);
        let stalled = start + Duration::from_secs(3);
        assert_eq!(next_deadline(start, TICK, false, stalled), stalled);
        assert_eq!(next_deadline(start, TICK, true, late), late);
    }
}
