use bevy::prelude::*;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheRadius, ClientConnection, JoinedGame, ServerProfile,
};
use mcrs_minecraft_network::columns::ColumnStore;
use mcrs_voxel_world::world::lifecycle::trace::{self, ColumnSample, ColumnStage};

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
    mut samples: Local<Vec<ColumnSample>>,
    mut hops: Local<Vec<f32>>,
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
    displayer.add_line(hop_medians(&mut samples, &mut hops));
}

/// The median time a column that reached the screen spent getting into each stage from the
/// one before it, over the columns still traced.
fn hop_medians(samples: &mut Vec<ColumnSample>, hops: &mut Vec<f32>) -> String {
    trace::snapshot(samples);
    let mut line = String::from("Hops p50 ms:");
    let mut meshed = 0;
    for stage in ColumnStage::ALL.into_iter().skip(1) {
        hops.clear();
        hops.extend(
            samples
                .iter()
                .filter(|sample| sample.stage == ColumnStage::Meshed)
                .filter_map(|sample| sample.hop(stage))
                .map(|hop| hop.as_secs_f32() * 1000.0),
        );
        meshed = hops.len();
        if hops.is_empty() {
            continue;
        }
        let at = hops.len() / 2;
        let median = *hops.select_nth_unstable_by(at, f32::total_cmp).1;
        line.push_str(&format!(" {} {median:.0}", stage.label()));
    }
    line.push_str(&format!(" over {meshed}"));
    line
}

fn phase(state: &ConnectionState) -> &'static str {
    match state {
        ConnectionState::Login => "logging in",
        ConnectionState::Configuration => "configuring",
        ConnectionState::Game => "playing",
    }
}
