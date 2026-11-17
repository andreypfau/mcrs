use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use mcrs_minecraft_engine_entity_motion::MotionHold;
use mcrs_minecraft_engine_entity_motion::MotionState;

use crate::PublishedTile;

pub(crate) fn hold_unseen(added: On<Add<MotionState>>, mut commands: Commands) {
    commands
        .entity(added.entity)
        .try_insert((MotionHold, PublishedTile::default()));
}
