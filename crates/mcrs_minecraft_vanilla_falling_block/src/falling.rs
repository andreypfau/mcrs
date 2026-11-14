use bevy_ecs::component::Component;
use mcrs_minecraft_falling_block::Fall;

/// The values of a falling block beside its `MotionState`, which holds its position, its velocity
/// and its age (FBK-DAT-001). The plugin ends the entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Falling(pub Fall);
