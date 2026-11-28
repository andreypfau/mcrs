use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;

use crate::player::Player;

#[derive(Copy, Clone, Debug, PartialEq, Component, Deref)]
pub struct PreviousPosition(pub(crate) DVec3);

pub(crate) fn capture_previous_position(
    player: Single<(&PhysicsTransform, &mut PreviousPosition), With<Player>>,
) {
    let (transform, mut previous) = player.into_inner();
    previous.set_if_neq(PreviousPosition(transform.translation));
}
