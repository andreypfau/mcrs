use crate::joined_game::CurrentDimension;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_network::client::ClientConnection;

use crate::stream::Loader;

pub fn record_traces(
    mut loader: ResMut<Loader>,
    traces: Option<Res<ColumnTraceSink>>,
    current: Option<Single<&CurrentDimension, With<ClientConnection>>>,
) {
    match (&traces, &current) {
        (Some(traces), Some(current)) => {
            traces.record(current.key.as_str(), loader.trace.drain(..))
        }
        _ => loader.trace.clear(),
    }
}
