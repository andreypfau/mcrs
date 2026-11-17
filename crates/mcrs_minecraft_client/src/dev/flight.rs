use bevy::prelude::*;
use mcrs_minecraft_client_world::local_motion::LocalMotion;
use mcrs_minecraft_client_world::movement_input::MovementInput;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform as PhysicsTransform};
use mcrs_minecraft_game_mode::Abilities;

#[derive(Resource)]
pub struct ScriptedFlight {
    pub flying_speed: f32,
    pub turn_at: Option<f32>,
    pub turned: bool,
}

pub(crate) fn fly_scripted(
    mut scripted: ResMut<ScriptedFlight>,
    time: Res<Time>,
    mut input: ResMut<MovementInput>,
    player: Single<(&mut PhysicsTransform, &mut Abilities, &mut LocalMotion), With<Player>>,
) {
    let (mut transform, mut abilities, motion) = player.into_inner();
    if let Some(turn_at) = scripted.turn_at
        && !scripted.turned
        && time.elapsed_secs() >= turn_at
    {
        scripted.turned = true;
        let rotation = transform.rotation;
        transform.rotation = Rotation::new(rotation.yaw() + 180.0, rotation.pitch());
    }
    let flying = Abilities {
        may_fly: true,
        flying_speed: scripted.flying_speed,
        ..*abilities
    };
    abilities.set_if_neq(flying);
    motion
        .map_unchanged(|motion| &mut motion.0.flight0)
        .set_if_neq(true);
    input.set_if_neq(MovementInput {
        forward: true,
        sprint: true,
        ..MovementInput::EMPTY
    });
}
