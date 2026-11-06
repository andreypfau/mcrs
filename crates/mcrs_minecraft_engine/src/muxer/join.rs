use bevy_ecs::message::MessageReader;
use bevy_ecs::system::{Query, Res, ResMut};
use mcrs_minecraft_engine_world::session::{Place, SessionAnchors, SessionPlacement};

use crate::channel_types::ToDim;
use crate::muxer::dim_inputs::DimInputs;
use crate::to_muxer::JoinPlaced;

/// Sends the spawn of each player that the coordinator placed into its dimension's next tick,
/// and marks its session as joining that dimension.
pub fn admit_placed_players(
    mut placed: MessageReader<JoinPlaced>,
    anchors: Res<SessionAnchors>,
    mut placements: Query<&mut SessionPlacement>,
    mut dim_inputs: ResMut<DimInputs>,
) {
    for JoinPlaced { dim, spawn } in placed.read() {
        let Some(anchor) = anchors.get(spawn.session) else {
            continue;
        };
        let Ok(mut placement) = placements.get_mut(anchor) else {
            continue;
        };
        if placement.place() != Place::Unplaced {
            continue;
        }
        if dim_inputs.send_control(*dim, ToDim::Spawn(spawn.clone())) {
            placement.set(Place::Joining(*dim));
        }
    }
}
