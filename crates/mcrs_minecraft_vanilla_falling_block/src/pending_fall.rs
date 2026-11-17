use bevy_ecs::component::Component;
use mcrs_minecraft_falling_block::Start;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PendingFall(pub Start);
