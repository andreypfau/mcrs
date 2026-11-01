use bevy::math::DVec3;
use bevy::prelude::*;

use mcrs_minecraft_engine_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};
use mcrs_minecraft_item::{SelectedHotbarSlot, SlotTable, slots};

use crate::local_player::{Flying, FlyingSpeed, LastSentMovement, Sprint};

#[derive(Component)]
pub struct Player;

pub fn spawn_player(world: &mut World, position: DVec3, yaw: f32, pitch: f32) -> Entity {
    let physics =
        PhysicsTransform::from_translation(position).with_rotation(Rotation::new(yaw, pitch));
    world
        .spawn((
            Player,
            physics,
            OldTransform(physics),
            Velocity(DVec3::ZERO),
            Flying,
            FlyingSpeed::default(),
            Sprint::default(),
            LastSentMovement::default(),
            SlotTable::fixed(slots::COUNT),
            SelectedHotbarSlot::default(),
        ))
        .id()
}
