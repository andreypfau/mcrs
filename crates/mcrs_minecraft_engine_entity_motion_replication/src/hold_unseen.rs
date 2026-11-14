use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use mcrs_minecraft_engine_entity_motion::MotionHold;
use mcrs_minecraft_engine_entity_motion::MotionState;

use crate::PublishedTile;

/// Holds each new motion until its id is assigned, so that its viewers see its spawn before any
/// move. An observer of the first insert, and not a required component, so that a later insert
/// of the state does not hold an entity that already has its id.
pub(crate) fn hold_unseen(added: On<Add<MotionState>>, mut commands: Commands) {
    commands
        .entity(added.entity)
        .try_insert((MotionHold, PublishedTile::default()));
}
