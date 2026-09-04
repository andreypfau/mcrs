use bevy::prelude::*;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheRadius, ClientConnection, JoinedGame, ServerProfile,
};
use mcrs_minecraft_network::columns::ColumnStore;

use super::DebugScreenDisplayer;

type Connection<'a> = (
    &'a ServerProfile,
    &'a ConnectionState,
    Option<&'a JoinedGame>,
    Option<&'a ChunkCacheRadius>,
);

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    connection: Option<Single<Connection, With<ClientConnection>>>,
    store: Option<Res<ColumnStore>>,
) {
    let Some(connection) = connection else {
        displayer.add_line("Server: not connected".to_owned());
        return;
    };
    let (profile, state, joined, radius) = *connection;
    let columns = store.map_or(0, |store| store.len());

    displayer.add_line(format!("Server: {} as {}", phase(state), profile.username));
    if let Some(joined) = joined {
        displayer.add_line(format!("Dimension: {}", joined.dimension));
    }
    displayer.add_line(match radius {
        Some(radius) => format!("Columns: {columns} resident, radius {}", radius.0),
        None => format!("Columns: {columns} resident"),
    });
}

fn phase(state: &ConnectionState) -> &'static str {
    match state {
        ConnectionState::Login => "logging in",
        ConnectionState::Configuration => "configuring",
        ConnectionState::Game => "playing",
    }
}
