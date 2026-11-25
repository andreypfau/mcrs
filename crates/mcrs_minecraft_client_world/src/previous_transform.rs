use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;

use crate::player::Player;

#[derive(Copy, Clone, Debug, PartialEq, Component, Deref)]
pub struct PreviousTransform(pub(crate) PhysicsTransform);

pub(crate) fn capture_previous_transform(
    player: Single<(&PhysicsTransform, &mut PreviousTransform), With<Player>>,
) {
    let (transform, mut previous) = player.into_inner();
    previous.set_if_neq(PreviousTransform(*transform));
}
