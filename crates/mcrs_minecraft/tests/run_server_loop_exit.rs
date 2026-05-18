use bevy_app::{App, AppExit};
use bevy_ecs::message::Messages;
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue};
use mcrs_minecraft::run_server_loop;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// `App` is not `Send` because it holds `Box<dyn FnOnce(App) -> AppExit>`.
// This wrapper is sound here because:
//   - the `App` is fully owned (no borrowed data, 'static lifetime),
//   - it is used only from one thread at a time,
//   - the spawning thread blocks on `join` before the wrapper is dropped.
//
// The wrapper must contain the `App` via a method, not a field access in
// the closure, because Rust 2021 edition closure capture captures individual
// fields, which would expose `App` (not `SendableApp`) to the `Send` check.
struct SendableApp(App);
unsafe impl Send for SendableApp {}

impl SendableApp {
    fn into_inner(self) -> App {
        self.0
    }
}

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
    let wrapper = SendableApp(app);

    let handle = thread::spawn(move || {
        run_server_loop(wrapper.into_inner());
        tx.send(()).ok();
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(()) => {
            handle.join().expect("loop thread joined");
        }
        Err(_) => panic!("run_server_loop did not exit within 5s after AppExit was written"),
    }
}
