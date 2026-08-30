use bevy::prelude::*;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheRadius, ClientConnection, JoinedGame, ReceivedChunkColumns, ServerProfile,
};

use super::DebugScreenDisplayer;

type Connection<'a> = (
    &'a ServerProfile,
    &'a ConnectionState,
    &'a ReceivedChunkColumns,
    Option<&'a JoinedGame>,
    Option<&'a ChunkCacheRadius>,
);

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    connection: Option<Single<Connection, With<ClientConnection>>>,
) {
    let Some(connection) = connection else {
        displayer.add_line("Server: not connected".to_owned());
        return;
    };
    let (profile, state, columns, joined, radius) = *connection;

    displayer.add_line(format!("Server: {} as {}", phase(state), profile.username));
    if let Some(joined) = joined {
        displayer.add_line(format!("Dimension: {}", joined.dimension));
    }
    displayer.add_line(match radius {
        Some(radius) => format!("Columns: {} received, radius {}", columns.0, radius.0),
        None => format!("Columns: {} received", columns.0),
    });
}

fn phase(state: &ConnectionState) -> &'static str {
    match state {
        ConnectionState::Login => "logging in",
        ConnectionState::Configuration => "configuring",
        ConnectionState::Game => "playing",
    }
}
