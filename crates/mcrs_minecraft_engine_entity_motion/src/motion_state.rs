use bevy_ecs::component::Component;
use mcrs_minecraft_physics::EntityState;

use crate::StepOutcome;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[require(StepOutcome)]
pub struct MotionState(pub EntityState);
