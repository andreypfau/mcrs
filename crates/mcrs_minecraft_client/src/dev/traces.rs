use bevy::prelude::*;
use mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_network::client::{ClientConnection, JoinedGame};

use crate::stream::Loader;

pub fn record_traces(
    mut loader: ResMut<Loader>,
    traces: Option<Res<ColumnTraceSink>>,
    joined: Option<Single<&JoinedGame, With<ClientConnection>>>,
) {
    match (&traces, &joined) {
        (Some(traces), Some(joined)) => {
            traces.record(joined.dimension.as_str(), loader.trace.drain(..))
        }
        _ => loader.trace.clear(),
    }
}
