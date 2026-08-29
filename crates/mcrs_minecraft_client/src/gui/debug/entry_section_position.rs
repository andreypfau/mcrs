use bevy::prelude::*;
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_world::entity::physics::Transform as PhysicsTransform;

use super::{DebugScreenDisplayer, entry_position};
use crate::player::Player;

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    camera: Single<&PhysicsTransform, With<Player>>,
) {
    let feet = BlockPos::from(camera.translation);
    displayer.add_to_group(
        entry_position::GROUP,
        [format!(
            "Section-relative: {:02} {:02} {:02}",
            feet.x & 15,
            feet.y & 15,
            feet.z & 15
        )],
    );
}
