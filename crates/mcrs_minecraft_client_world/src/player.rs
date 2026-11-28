use bevy::math::DVec3;
use bevy::prelude::*;

use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_world::entity::physics::{
    Rotation, Transform as PhysicsTransform, Velocity,
};

use crate::hotbar_sync::LastSentHotbarSlot;
use crate::local_player::{LastSentMovement, Sprint};
use crate::player_load::PlayerLoad;
use crate::previous_position::PreviousPosition;

#[derive(Component)]
#[require(LastSentHotbarSlot, PlayerLoad)]
pub struct Player;

pub fn look_direction(yaw: f32, pitch: f32) -> Vec3 {
    let (pitch, yaw) = (pitch.to_radians(), -yaw.to_radians());
    Vec3::new(
        yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

pub fn player_at(position: DVec3, yaw: f32, pitch: f32) -> impl Bundle {
    let physics =
        PhysicsTransform::from_translation(position).with_rotation(Rotation::new(yaw, pitch));
    (
        Player,
        physics,
        PreviousPosition(position),
        Velocity(DVec3::ZERO),
        Sprint::default(),
        LastSentMovement::default(),
        Selected::default(),
    )
}
