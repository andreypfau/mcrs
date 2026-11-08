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
    /// Whether this app may size the task pools. They are process-global and
    /// `get_or_init`, so an app embedded beside another one must take the
    /// defaults rather than shrink pools the host is already using.
    pub owns_task_pools: bool,
    /// Absolute path to the asset corpus. `None` leaves bevy to resolve
    /// `assets` against `BEVY_ASSET_ROOT` or the executable directory, which a
    /// server embedded in another process cannot rely on.
    pub asset_path: Option<String>,
    /// Where the dimension worlds tick, as [`WorldPlacement`] says. Under
    /// [`Placement::Inline`] an app that does not own the task pools still runs `par_iter` on the
    /// host's compute pool.
    pub worlds: Placement,
    /// The thread count of the compute pool, for an app that owns the task pools. `None` keeps
    /// one thread under [`Placement::Inline`] and the Bevy default otherwise.
    pub compute_threads: Option<NonZeroUsize>,
}

impl Plugin for VoxelServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldPlacement(self.worlds));
        let compute_threads = match (self.compute_threads, self.worlds) {
            _ if !self.owns_task_pools => None,
            (Some(threads), _) => Some(threads.get()),
            (None, Placement::Inline) => Some(1),
            (None, _) => None,
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
        // One host update is one tick: the host's fixed schedules run exactly once in it, as
        // the dimensions' do, however late the update starts. Wall-clock time decides only
        // when a tick starts, never what it does.
        app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        // Dropping a notify fsevents watcher joins its CFRunLoop thread and
        // can block forever. The server has no hot-reload consumer.
        let mut assets = AssetPlugin {
            watch_for_changes_override: Some(false),
            ..AssetPlugin::default()
        };
        if let Some(path) = &self.asset_path {
            assets.file_path = path.clone();
        }
        app.add_plugins(assets);
    }

    // After every plugin's `finish`, which may still add a schedule.
    fn cleanup(&self, app: &mut App) {
        if app.world().get_resource::<WorldPlacement>() == Some(&WorldPlacement(Placement::Inline))
        {
            run_schedules_on_one_thread(app.world_mut());
        }
    }
}

/// The longest backlog of ticks that the loop runs back to back to catch up.
const MAX_BACKLOG: Duration = Duration::from_secs(2);

/// When the next tick starts. The ticks keep a fixed schedule, one `tick` after another from the
/// first, so a late wake or a long tick does not shift the ticks after it: a late tick is followed
/// at once by the next. A backlog longer than [`MAX_BACKLOG`] is dropped, and the schedule starts
/// again from `now`. A tick of a sprint, `at_once`, starts the schedule again from `now` too.
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

/// Gives every schedule of `world` a single-threaded executor.
pub fn run_schedules_on_one_thread(world: &mut World) {
    let Some(mut schedules) = world.get_resource_mut::<Schedules>() else {
        return;
    };
    for (_, schedule) in schedules.iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
}

/// `App` is `!Send` only because it owns a boxed runner closure. Handing a
/// whole, unaliased `App` to one other thread is sound. The wrapper must be
/// moved by method call, not field access, or disjoint closure capture takes
/// the `App` itself and the `Send` bound fails again.
struct SendableApp(App);

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
    let app = SendableApp(app);
    std::thread::Builder::new()
        .name("voxel-server".to_string())
        .spawn(move || run(app.into_inner()))
}

/// Ticks the app at `tick_rate`. `between_ticks` runs once after every tick. `until` then has
/// the time up to the next tick, given as its deadline, to pass on what the tick wrote for the
/// clients. It must not change what a tick reads, and it returns at the deadline: the loop
/// starts the next tick as soon as it returns. A plugin that asks the app to exit
/// while it finishes stops the loop before the first tick.
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
        let at_once = app
            .world_mut()
            .get_resource_mut::<TickPace>()
            .is_some_and(|mut pace| pace.end_tick(nanos));
        next = next_deadline(next, tick, at_once, Instant::now());
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
