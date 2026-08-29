pub mod dim;
pub mod packet;

use bevy_app::prelude::*;
use bevy_app::{App, Plugin, TaskPoolOptions, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};
use bevy_time::{Fixed, Time, TimePlugin};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

pub struct VoxelServerPlugin {
    pub tick_rate: NonZeroU32,
    /// Whether this app may size the task pools. They are process-global and
    /// `get_or_init`, so an app embedded beside another one must take the
    /// defaults rather than shrink pools the host is already using.
    pub owns_task_pools: bool,
}

impl Plugin for VoxelServerPlugin {
    fn build(&self, app: &mut App) {
        if self.owns_task_pools && cfg!(debug_assertions) {
            app.add_plugins(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions::with_num_threads(1),
            });
        } else {
            app.add_plugins(TaskPoolPlugin::default());
        }

        app.edit_schedule(Update, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        });
        #[cfg(debug_assertions)]
        force_singlethread_schedules(app);

        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        app.insert_resource(Time::<Fixed>::from_hz(self.tick_rate.get() as f64));
        // Dropping a notify fsevents watcher joins its CFRunLoop thread and
        // can block forever. The server has no hot-reload consumer.
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..AssetPlugin::default()
        });
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
) -> std::thread::JoinHandle<()> {
    let app = SendableApp(app);
    std::thread::Builder::new()
        .name("voxel-server".to_string())
        .spawn(move || run(app.into_inner()))
        .expect("failed to spawn the server thread")
}

pub fn run_server_loop(
    mut app: App,
    tick_rate: NonZeroU32,
    mut between_ticks: impl FnMut(&mut App),
) {
    let tick = Duration::from_secs_f64(1.0 / tick_rate.get() as f64);
    app.finish();
    app.cleanup();
    loop {
        let start = Instant::now();
        app.update();
        between_ticks(&mut app);
        if app.should_exit().is_some() {
            break;
        }
        let elapsed = start.elapsed();
        if elapsed < tick {
            std::thread::sleep(tick - elapsed);
        }
    }
}

#[cfg(debug_assertions)]
fn force_singlethread_schedules(app: &mut App) {
    for label in [
        PreStartup.intern(),
        Startup.intern(),
        PostStartup.intern(),
        First.intern(),
        PreUpdate.intern(),
        RunFixedMainLoop.intern(),
        Update.intern(),
        PostUpdate.intern(),
        Last.intern(),
        FixedFirst.intern(),
        FixedPreUpdate.intern(),
        FixedUpdate.intern(),
        FixedPostUpdate.intern(),
        FixedLast.intern(),
    ] {
        app.edit_schedule(label, |s| {
            s.set_executor(SingleThreadedExecutor::new());
        });
    }
}
