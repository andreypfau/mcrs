use bevy_ecs::component::Component;
use mcrs_minecraft_physics::MotionRow;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MotionKind(pub MotionRow);
