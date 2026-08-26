use bevy_ecs_macros::Component;

pub mod chunk_view;
pub mod reposition;

#[derive(Component, Default)]
pub struct Player;
