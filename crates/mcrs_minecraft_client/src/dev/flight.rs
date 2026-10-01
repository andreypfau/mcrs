use bevy::prelude::*;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform as PhysicsTransform};
use mcrs_minecraft_world::entity::player::Input;

#[derive(Resource)]
pub struct ScriptedFlight {
    pub turn_at: Option<f32>,
    pub turned: bool,
}

impl ScriptedFlight {
    pub fn input(&mut self, transform: &mut PhysicsTransform, elapsed: f32) -> Input {
        if let Some(turn_at) = self.turn_at
            && !self.turned
            && elapsed >= turn_at
        {
            self.turned = true;
            let rotation = transform.rotation;
            transform.rotation = Rotation::new(rotation.yaw() + 180.0, rotation.pitch());
        }
        Input {
            forward: true,
            sprint: true,
            ..Input::EMPTY
        }
    }
}
