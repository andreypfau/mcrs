use crate::movement_input::MovementInput;
use crate::player::Player;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform as PhysicsTransform};

#[derive(Resource)]
pub struct ScriptedFlight {
    pub turn_at: Option<f32>,
    pub turned: bool,
}

pub(crate) fn fly_scripted(
    mut scripted: ResMut<ScriptedFlight>,
    time: Res<Time>,
    mut input: ResMut<MovementInput>,
    mut transform: Single<&mut PhysicsTransform, With<Player>>,
) {
    if let Some(turn_at) = scripted.turn_at
        && !scripted.turned
        && time.elapsed_secs() >= turn_at
    {
        scripted.turned = true;
        let rotation = transform.rotation;
        transform.rotation = Rotation::new(rotation.yaw() + 180.0, rotation.pitch());
    }
    input.set_if_neq(MovementInput {
        forward: true,
        sprint: true,
        ..MovementInput::EMPTY
    });
}
