use bevy_app::{App, AppExit, Update};
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_server::{BoundAddress, MinecraftServerPlugin, run_server_loop};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn embedded_server_accepts_a_loopback_connection_and_stops() {
    let mut app = App::new();
    app.add_plugins(MinecraftServerPlugin::embedded());

    let address = app.world().resource::<BoundAddress>().0;
    assert!(address.ip().is_loopback(), "embedded server left loopback");
    assert_ne!(address.port(), 0, "the OS-assigned port was not reported");

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    app.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stop_flag.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });

    let (finished_send, finished_recv) = mpsc::channel();
    let handle = mcrs_minecraft_server::spawn_server_thread(app, move |app| {
        run_server_loop(app);
        finished_send.send(()).ok();
    });

    let connection = TcpStream::connect(address).expect("connect to the embedded server");
    drop(connection);

    stop.store(true, Ordering::Relaxed);
    finished_recv
        .recv_timeout(Duration::from_secs(30))
        .expect("the embedded server did not stop after AppExit");
    handle.join().expect("server thread joined");
}
