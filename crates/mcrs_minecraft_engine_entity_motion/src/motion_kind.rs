use bevy_ecs::component::Component;
use mcrs_minecraft_physics::MotionRow;

/// The motion parameters of the category of an entity, which do not change while it moves. The
/// pass does not end an entity at the lifetime of the row; the kind plugin does.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MotionKind(pub MotionRow);
