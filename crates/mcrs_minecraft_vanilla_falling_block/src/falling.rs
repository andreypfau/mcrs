use bevy_ecs::component::Component;
use mcrs_minecraft_falling_block::Fall;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Falling(pub Fall);
