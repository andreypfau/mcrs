use bevy_ecs::component::Component;
use mcrs_minecraft_physics::EntityState;

use crate::StepOutcome;

/// The authoritative motion of an entity: its key, body, rest and age. Inside
/// [`MotionPass`](crate::MotionPass) only the pass writes it, and it marks the component changed
/// only when the body changed. A write from another system wakes a resting entity.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[require(StepOutcome)]
pub struct MotionState(pub EntityState);
