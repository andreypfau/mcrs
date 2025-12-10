use crate::chunk_render_debug::ChunkRenderDebug;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::{DefaultPlugins, MinimalPlugins};
use bevy::log::{tracing, LogPlugin};
use bevy_app::{App, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ExecutorKind;
use mcrs_network::EngineConnection;
use mcrs_protocol::WritePacket;
use mcrs_server::ServerPlugin;
use tokio::io::AsyncReadExt;

mod chunk_render_debug;

#[tokio::main]
async fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin::default())
        .add_systems(Startup, setup)
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        // .add_plugins(ChunkRenderDebug)
        // .add_plugins(ScheduleRunnerPlugin::default())
        .add_plugins(ServerPlugin)
        .edit_schedule(Update, |s| { s.set_executor_kind(ExecutorKind::SingleThreaded); })
        .edit_schedule(FixedPreUpdate, |s| { s.set_executor_kind(ExecutorKind::SingleThreaded); })
        .edit_schedule(FixedUpdate, |s| { s.set_executor_kind(ExecutorKind::SingleThreaded); })
        .edit_schedule(FixedPostUpdate, |s| { s.set_executor_kind(ExecutorKind::SingleThreaded); })
        .edit_schedule(FixedLast, |s| { s.set_executor_kind(ExecutorKind::SingleThreaded); })
        .add_systems(FixedLast, tick_system)
        .run();
    println!("Hello, world!");
}

fn setup(mut commands: Commands) {}

fn tick_system() {
    tracing::event!(
            tracing::Level::INFO,
            message = "finished frame",
            tracy.frame_mark = true
        );
}