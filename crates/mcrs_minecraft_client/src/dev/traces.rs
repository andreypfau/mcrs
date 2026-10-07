use bevy::prelude::*;
use mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_network::client::{ClientConnection, CurrentDimension};

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
