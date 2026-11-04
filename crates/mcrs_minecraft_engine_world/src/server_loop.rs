use bevy_app::prelude::*;
use bevy_app::{App, Plugin, TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use bevy_asset::AssetPlugin;
use bevy_ecs::schedule::SingleThreadedExecutor;
use bevy_time::{Fixed, Time, TimePlugin};
use rayon::ThreadPoolBuilder;
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use crate::tick_threading::{self, DimTickPool, TickThreading};

/// One thread for each vanilla dimension. A dimension's executor blocks its thread for the whole
/// tick, so a thread more than there are dimensions never works, and further dimensions take turns.
const DIM_TICK_THREADS: usize = 3;

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
    /// How the game logic of a tick uses threads. In [`TickThreading::Single`] an app that does
    /// not own the task pools still runs `par_iter` on the host's compute pool.
    pub threading: TickThreading,
}

impl Plugin for VoxelServerPlugin {
    fn build(&self, app: &mut App) {
        let threading = self.threading;
        if threading == TickThreading::Parallel {
            match ThreadPoolBuilder::new()
                .num_threads(DIM_TICK_THREADS)
                .thread_name(|index| format!("dim-tick-{index}"))
                .build()
            {
                Ok(pool) => {
                    app.insert_resource(DimTickPool(pool));
                }
                Err(error) => {
                    tracing::error!(%error, "the dimension tick pool did not start; the dimensions tick one after another, each with its systems in parallel");
                }
            }
        }
        app.insert_resource(threading);
        let task_pool_options = if threading == TickThreading::Single && self.owns_task_pools {
            TaskPoolOptions {
                compute: TaskPoolThreadAssignmentPolicy {
                    min_threads: 1,
                    max_threads: 1,
                    ..TaskPoolOptions::default().compute
                },
                ..TaskPoolOptions::default()
            }
        } else {
            TaskPoolOptions::default()
        };
        app.add_plugins(TaskPoolPlugin { task_pool_options });

        app.edit_schedule(Update, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        });

        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        app.insert_resource(Time::<Fixed>::from_hz(self.tick_rate.get() as f64));
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
        if app.world().get_resource::<TickThreading>() == Some(&TickThreading::Single) {
            tick_threading::run_schedules_on_one_thread(app.world_mut());
        }
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
    loop {
        let start = Instant::now();
        tick_threading::update(&mut app);
        between_ticks(&mut app);
        if let Some(exit) = app.should_exit() {
            return exit;
        }
        until(&mut app, start + tick);
    }
}
