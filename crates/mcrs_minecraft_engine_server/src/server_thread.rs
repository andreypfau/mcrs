use std::net::SocketAddr;
use std::thread::JoinHandle;

use bevy_app::{App, AppExit, Update};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::world::World;
use mcrs_minecraft_network::BoundAddress;
use tokio::sync::oneshot;

use crate::WorldOpenError;
use crate::run_server_loop_with;
use crate::shutdown::{ShutdownPlugin, ShutdownSignal, forward_shutdown, shutdown_system};
use crate::world_options::FailedStart;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ServerThreadError<E> {
    #[error("the server app could not be built")]
    Build(#[source] E),
    #[error("the server thread did not start: {0}")]
    Spawn(std::io::ErrorKind),
    #[error("the server app has no ShutdownPlugin, so a shutdown request would never end it")]
    NoShutdownPlugin,
    #[error("the server app is not bound to an address")]
    NotBound,
    #[error("the server thread panicked")]
    Panicked,
    #[error("the world did not open: {0}")]
    World(#[source] WorldOpenError),
}

#[derive(Resource)]
struct FailedStartSender(oneshot::Sender<WorldOpenError>);

pub struct ServerThread<E> {
    bound: oneshot::Receiver<SocketAddr>,
    address: Option<SocketAddr>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<Result<AppExit, ServerThreadError<E>>>>,
}

impl<E: Send + 'static> ServerThread<E> {
    pub fn start(
        build: impl FnOnce() -> Result<App, E> + Send + 'static,
    ) -> Result<Self, ServerThreadError<E>> {
        let (bound_send, bound) = oneshot::channel();
        let (stop, stop_receive) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("mcrs-server".to_owned())
            .spawn(move || {
                let mut app = build().map_err(ServerThreadError::Build)?;
                if let Some(FailedStart(error)) = app.world_mut().remove_resource::<FailedStart>() {
                    return Err(ServerThreadError::World(error));
                }
                if !app.is_plugin_added::<ShutdownPlugin>() {
                    return Err(ServerThreadError::NoShutdownPlugin);
                }
                let address = app
                    .world()
                    .get_resource::<BoundAddress>()
                    .ok_or(ServerThreadError::NotBound)?
                    .0;
                app.insert_resource(ShutdownSignal(stop_receive));
                app.add_systems(
                    Update,
                    forward_shutdown
                        .run_if(resource_exists::<ShutdownSignal>)
                        .before(shutdown_system),
                );
                let (failed_send, mut failed) = oneshot::channel();
                app.insert_resource(FailedStartSender(failed_send));
                bound_send.send(address).ok();
                let exit = run_server_loop_with(app, forward_failed_start);
                match failed.try_recv() {
                    Ok(error) => Err(ServerThreadError::World(error)),
                    Err(_) => Ok(exit),
                }
            })
            .map_err(|error| ServerThreadError::Spawn(error.kind()))?;
        Ok(Self {
            bound,
            address: None,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    pub fn address(&mut self) -> Option<SocketAddr> {
        if self.address.is_none() {
            self.address = self.bound.try_recv().ok();
        }
        self.address
    }

    pub fn request_shutdown(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.send(()).ok();
        }
    }

    pub fn try_finish(&mut self) -> Option<Result<AppExit, ServerThreadError<E>>> {
        let thread = self.thread.take_if(|thread| thread.is_finished())?;
        Some(thread.join().unwrap_or(Err(ServerThreadError::Panicked)))
    }

    pub fn finish(&mut self) -> Option<Result<AppExit, ServerThreadError<E>>> {
        self.request_shutdown();
        let thread = self.thread.take()?;
        Some(thread.join().unwrap_or(Err(ServerThreadError::Panicked)))
    }
}

fn forward_failed_start(world: &mut World) {
    let Some(FailedStart(error)) = world.remove_resource::<FailedStart>() else {
        return;
    };
    if let Some(FailedStartSender(sender)) = world.remove_resource::<FailedStartSender>() {
        sender.send(error).ok();
    }
}
