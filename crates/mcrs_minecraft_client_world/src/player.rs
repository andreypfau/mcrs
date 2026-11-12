use bevy::math::DVec3;
use bevy::prelude::*;

use mcrs_minecraft_engine_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};
use mcrs_minecraft_item::{SelectedHotbarSlot, SlotTable, slots};

use crate::local_player::{Flying, FlyingSpeed, LastSentMovement, Sprint};

#[derive(Component)]
pub struct Player;

/// The unit vector a look of `yaw` and `pitch`, in Minecraft degrees, points along: yaw 0 faces
/// +Z, yaw 90 faces -X, and a positive pitch looks down.
pub fn look_direction(yaw: f32, pitch: f32) -> Vec3 {
    let (pitch, yaw) = (pitch.to_radians(), -yaw.to_radians());
    Vec3::new(
        yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

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
            crate::vitals::LocalVitals::default(),
        ))
        .id()
}
