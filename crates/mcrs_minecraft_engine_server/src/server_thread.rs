use std::net::SocketAddr;
use std::thread::JoinHandle;

use bevy_app::{App, AppExit, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::resource_exists;
use mcrs_minecraft_network::BoundAddress;
use tokio::sync::oneshot;

use crate::run_server_loop;
use crate::shutdown::{ShutdownRequest, ShutdownSignal, forward_shutdown};

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ServerThreadError<E> {
    #[error("the server app could not be built")]
    Build(#[source] E),
    #[error("the server thread did not start: {0}")]
    Spawn(std::io::ErrorKind),
    #[error("the server app is not bound to an address")]
    NotBound,
    #[error("the server thread panicked")]
    Panicked,
}

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
                let address = app
                    .world()
                    .get_resource::<BoundAddress>()
                    .ok_or(ServerThreadError::NotBound)?
                    .0;
                app.add_message::<ShutdownRequest>();
                app.insert_resource(ShutdownSignal(stop_receive));
                app.add_systems(
                    Update,
                    forward_shutdown.run_if(resource_exists::<ShutdownSignal>),
                );
                bound_send.send(address).ok();
                Ok(run_server_loop(app))
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
