use bevy_ecs::component::Component;

pub mod chunk_view;
pub mod reposition;

#[derive(Component, Default)]
pub struct Player;
