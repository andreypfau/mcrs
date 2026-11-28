use std::fmt;
use std::path::PathBuf;

use bevy::prelude::*;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::ConnectionEnded;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol_game::game_event::GameEventKind;
use mcrs_minecraft_save::SaveLock;

use crate::registries::entered_configuration;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[relationship(relationship_target = SessionEntities)]
pub struct InSession(pub Entity);

#[derive(Component, Debug)]
#[relationship_target(relationship = InSession, linked_spawn)]
pub struct SessionEntities(Vec<Entity>);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct LevelChunksLoadStarted;

pub fn session_boundary(
    connections: Query<&ConnectionState, Changed<ConnectionState>>,
    mut ended: MessageReader<ConnectionEnded>,
) -> bool {
    let session_ended = !ended.is_empty();
    ended.clear();
    session_ended || entered_configuration(connections)
}

pub(crate) fn add_observers(app: &mut App) {
    app.add_observer(on_level_chunks_load_start);
}

fn on_level_chunks_load_start(event: On<Received<ClientboundGameEvent>>, mut commands: Commands) {
    if let Some(packet) = event.packet()
        && matches!(packet.game_event, GameEventKind::LevelChunksLoadStart)
    {
        commands
            .entity(event.entity)
            .try_insert(LevelChunksLoadStarted);
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalServer {
    pub phase: LocalServerPhase,
    pub origin: LocalWorldOrigin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalServerPhase {
    Starting,
    Running,
    Stopping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalWorldOrigin {
    Opened,
    Created,
}

#[derive(Message, Debug)]
pub struct OpenLocalWorld {
    pub folder: PathBuf,
    pub host: Option<Uuid>,
    pub origin: LocalWorldOrigin,
    pub lock: Option<SaveLock>,
}

#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct LocalServerStopped(pub Result<(), LocalServerError>);

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalServerError {
    Held,
    Lock(std::io::ErrorKind),
    OpsFile,
    Spawn(std::io::ErrorKind),
    NotBound,
    Exited(u8),
    Panicked,
    World(LocalWorldError),
    Busy,
}

impl fmt::Display for LocalServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held => f.write_str("the world is open in another process"),
            Self::Lock(kind) => write!(f, "the world folder could not be locked: {kind}"),
            Self::OpsFile => f.write_str("the operator file could not be read"),
            Self::Spawn(kind) => write!(f, "the server thread did not start: {kind}"),
            Self::NotBound => f.write_str("the server listens on no address"),
            Self::Exited(code) => write!(f, "the server stopped with error {code}"),
            Self::Panicked => f.write_str("the server crashed"),
            Self::World(error) => error.fmt(f),
            Self::Busy => f.write_str("another local world is already opening"),
        }
    }
}

impl std::error::Error for LocalServerError {}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalWorldError {
    DataVersion { found: Option<i32>, expected: i32 },
    Files,
    Dimensions,
}

impl fmt::Display for LocalWorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataVersion {
                found: Some(found),
                expected,
            } => write!(f, "the world has data version {found}, expected {expected}"),
            Self::DataVersion {
                found: None,
                expected,
            } => write!(f, "the world has no data version, expected {expected}"),
            Self::Files => f.write_str("the files of the world could not be read or written"),
            Self::Dimensions => {
                f.write_str("the dimensions of the world do not match the data pack")
            }
        }
    }
}

impl std::error::Error for LocalWorldError {}
