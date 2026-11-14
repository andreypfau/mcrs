use bevy_ecs::component::Component;
use mcrs_minecraft_falling_block::FallingBlock;

/// The values of a falling block beside its `MotionState`, which holds its position and velocity
/// (FBK-DAT-001). The plugin ages it before the motion pass and ends the entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FallingBlockEntity(pub FallingBlock);
