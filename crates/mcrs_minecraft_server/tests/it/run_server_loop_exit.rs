use bevy_app::{App, AppExit, Plugin, Startup};
use bevy_ecs::message::Messages;
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue};
use mcrs_minecraft_server::{run_server_loop, spawn_server_thread};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn run_server_loop_exits_on_app_exit() {
    let mut app = App::new();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    app.add_message::<AppExit>();
    app.world_mut()
        .resource_mut::<Messages<AppExit>>()
        .write(AppExit::Success);

    let (tx, rx) = mpsc::channel::<()>();

    let handle = spawn_server_thread(app, move |app| {
        run_server_loop(app);
        tx.send(()).ok();
    })
    .expect("the loop thread starts");

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(()) => {
            handle.join().expect("loop thread joined");
        }
        Err(_) => panic!("run_server_loop did not exit within 5s after AppExit was written"),
    }
}

struct ExitsWhileFinishing;

impl Plugin for ExitsWhileFinishing {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        app.world_mut().write_message(AppExit::error());
    }
}

#[test]
fn an_exit_asked_for_while_finishing_stops_the_loop_before_the_first_tick() {
    let mut app = App::new();
    app.add_plugins(ExitsWhileFinishing);
    app.add_systems(Startup, || panic!("the app ticked"));

    assert_eq!(run_server_loop(app), AppExit::error());
}
