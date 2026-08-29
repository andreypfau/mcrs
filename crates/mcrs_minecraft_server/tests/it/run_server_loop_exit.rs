use bevy_app::{App, AppExit};
use bevy_ecs::message::Messages;
use mcrs_minecraft_server::{run_server_loop, spawn_server_thread};
use mcrs_voxel_world::world::sub_app::{DimDespawnQueue, DimSpawnQueue};
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
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(()) => {
            handle.join().expect("loop thread joined");
        }
        Err(_) => panic!("run_server_loop did not exit within 5s after AppExit was written"),
    }
}
