use bevy_app::ScheduleRunnerPlugin;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ExecutorKind, ScheduleLabel};
use bevy_log::{Level, LogPlugin};
use mcrs_minecraft::ServerPlugin;
use std::time::Duration;

mod chunk_render_debug;

const LOG_FILTER: &str =
    "mcrs_minecraft=debug,mcrs_minecraft::world::entity::player::digging=trace";

#[tokio::main]
async fn main() {
    let mut app = App::new();
    setup_schedules(&mut app);
    app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        1.0 / mcrs_minecraft::DEFAULT_TPS.get() as f64,
    )))
    .add_plugins(LogPlugin {
        filter: LOG_FILTER.to_string(),
        level: Level::INFO,
        ..Default::default()
    })
    .add_plugins(ServerPlugin)
    .run();
}

fn setup_schedules(app: &mut App) {
    #[cfg(debug_assertions)]
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: TaskPoolOptions::with_num_threads(1),
    });

    #[cfg(not(debug_assertions))]
    app.add_plugins(TaskPoolPlugin::default());

    app.edit_schedule(Update, |schedule| {
        schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    });
    #[cfg(debug_assertions)]
    force_singlethread_schedules(app);
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
            s.set_executor_kind(ExecutorKind::SingleThreaded);
        });
    }
}
