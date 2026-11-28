use crate::chunk_map::JoinedTraces;
use bevy::prelude::*;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_client_world::joined_game::CurrentDimension;
use mcrs_minecraft_client_world::player_view::ChunkCacheRadius;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnSample, ColumnStage};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientConnection, ServerProfile};

use crate::debug_displayer::DebugScreenDisplayer;

type Connection<'a> = (
    &'a ServerProfile,
    &'a ConnectionState,
    Option<&'a CurrentDimension>,
    Option<&'a ChunkCacheRadius>,
);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a debug screen refresh, at most ten times a second while the F3 overlay shows or when the stats log is due"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(9))]
pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    connection: Option<Single<Connection, With<ClientConnection>>>,
    store: Option<Res<ColumnStore>>,
    mut samples: Local<Vec<ColumnSample>>,
    mut hops: Local<Vec<f32>>,
    traces: JoinedTraces,
) {
    let Some(connection) = connection else {
        displayer.add_line("Server: not connected".to_owned());
        return;
    };
    let (profile, state, current, radius) = *connection;
    let columns = store.map_or(0, |store| store.len());

    displayer.add_line(format!("Server: {} as {}", phase(state), profile.username));
    if let Some(current) = current {
        displayer.add_line(format!("Dimension: {}", current.key));
    }
    displayer.add_line(match radius {
        Some(radius) => format!("Columns: {columns} resident, radius {}", radius.0),
        None => format!("Columns: {columns} resident"),
    });
    displayer.add_line(hop_medians(&traces, &mut samples, &mut hops));
}

fn hop_medians(
    traces: &JoinedTraces,
    samples: &mut Vec<ColumnSample>,
    hops: &mut Vec<f32>,
) -> String {
    traces.snapshot(samples);
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
        ConnectionState::Handshaking | ConnectionState::Status | ConnectionState::Login => {
            "logging in"
        }
        ConnectionState::Configuration => "configuring",
        ConnectionState::Game => "playing",
    }
}
