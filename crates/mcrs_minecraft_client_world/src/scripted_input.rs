use bevy::prelude::*;

use crate::attack_input::AttackInput;
use crate::movement_input::MovementInput;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptedInput {
    pub movement: MovementInput,
    pub attack: AttackInput,
}
