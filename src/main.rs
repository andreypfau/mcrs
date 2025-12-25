use bevy_app::ScheduleRunnerPlugin;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ExecutorKind, ScheduleLabel};
use bevy_log::LogPlugin;
use mcrs_minecraft::ServerPlugin;

mod chunk_render_debug;

#[tokio::main]
async fn main() {
    let mut app = App::new();
    setup_schedules(&mut app);
    app.add_plugins(ScheduleRunnerPlugin::default())
        .add_plugins(LogPlugin::default())
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
