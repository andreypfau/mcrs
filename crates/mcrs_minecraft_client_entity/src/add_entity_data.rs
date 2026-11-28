use bevy_ecs::component::Component;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddEntityData(pub i32);
