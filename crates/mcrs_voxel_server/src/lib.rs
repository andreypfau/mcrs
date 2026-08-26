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
}

impl Plugin for VoxelServerPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(debug_assertions)]
        app.add_plugins(TaskPoolPlugin {
            task_pool_options: TaskPoolOptions::with_num_threads(1),
        });
        #[cfg(not(debug_assertions))]
        app.add_plugins(TaskPoolPlugin::default());

        app.edit_schedule(Update, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        });
        #[cfg(debug_assertions)]
        force_singlethread_schedules(app);

        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        app.insert_resource(Time::<Fixed>::from_hz(self.tick_rate.get() as f64));
        app.add_plugins(AssetPlugin::default());
    }
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
