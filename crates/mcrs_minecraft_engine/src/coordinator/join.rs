use std::sync::Arc;

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::Res;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::bus::InboundPlayerSpawn;
use crate::coordinator::running_dimensions::RunningDimensions;
use crate::to_coordinator::JoinRequested;
use crate::to_muxer::JoinPlaced;

/// Places each joining player in the running dimension that its save names, or in the first
/// dimension of the list when the save names none or a dimension that does not run. A request
/// that finds no running dimension gets no answer, and the muxer asks again.
pub fn place_joining_players(
    mut requests: MessageReader<JoinRequested>,
    running: Res<RunningDimensions>,
    dimension_list: Res<DimensionList>,
    mut placed: MessageWriter<JoinPlaced>,
) {
    let live = |key: &ResourceKeyBuf<DimensionKey>| {
        running
            .iter()
            .find(|dimension| dimension.key == *key)
            .map(|dimension| dimension.label)
    };
    for request in requests.read() {
        let Some(dim) = request
            .saved_dimension
            .as_ref()
            .and_then(live)
            .or_else(|| live(dimension_list.keys().first()?))
        else {
            continue;
        };
        placed.write(JoinPlaced {
            dim,
            spawn: InboundPlayerSpawn {
                session: request.session,
                snapshot: request.snapshot.clone(),
                dimensions: Arc::clone(dimension_list.keys()),
            },
        });
    }
}
