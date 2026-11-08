use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;
use std::time::Duration;

use bevy_app::{App, AppExit, Update};
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::{
    BoundAddress, DataPack, DimensionWorldsPlugin, ServerCorePlugin, ServerNetworkPlugin,
    run_server_loop, spawn_server_thread,
};
use mcrs_minecraft_network::RawConnection;
use mcrs_minecraft_network::join::{Join, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::uuid::Uuid;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

/// The engine's own server on a thread, with no world generation, light, command or chat
/// plugin. Dropping it stops the server loop, a failed test included.
pub struct RunningServer {
    pub address: SocketAddr,
    stop: Arc<AtomicBool>,
    finished: Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl RunningServer {
    /// # Panics
    ///
    /// When the server thread does not start.
    pub fn start(network: ServerNetworkPlugin) -> Self {
        let mut app = App::new();
        app.add_plugins((
            ServerCorePlugin {
                owns_task_pools: false,
                asset_path: Some(ASSETS.to_owned()),
                data_pack: DataPack::Directory(mcrs_minecraft_fixtures::vanilla_dir().to_owned()),
                ..Default::default()
            },
            DimensionWorldsPlugin::default(),
            ServerNetworkPlugin {
                announce_on_lan: false,
                ..network
            },
        ));
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        app.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
            if stopped.load(Ordering::Relaxed) {
                exit.write(AppExit::Success);
            }
        });
        let address = app.world().resource::<BoundAddress>().0;
        let (finished_send, finished) = std::sync::mpsc::channel();
        let thread = spawn_server_thread(app, move |app| {
            run_server_loop(app);
            finished_send.send(()).ok();
        })
        .expect("the server thread starts");
        Self {
            address,
            stop,
            finished,
            thread: Some(thread),
        }
    }

    /// Joins a bot and answers its connection in the play state.
    ///
    /// # Panics
    ///
    /// When the join fails.
    pub async fn join(&self) -> RawConnection {
        let io = PacketIo::connect(self.address).await.unwrap();
        let joined = join(
            io,
            Join {
                server_address: self.address.ip().to_string(),
                server_port: self.address.port(),
                remote_addr: self.address,
                username: "Bot".to_owned(),
                profile_id: Uuid::nil(),
                protocol_version: VERSION.protocol_version,
                view_distance: 2,
                holds_vanilla_pack: false,
            },
        )
        .await
        .unwrap();
        joined.connection
    }

    /// # Panics
    ///
    /// When the server does not stop after `AppExit`.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.finished
            .recv_timeout(Duration::from_secs(30))
            .expect("the server did not stop after AppExit");
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
