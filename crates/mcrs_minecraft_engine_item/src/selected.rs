use bevy_ecs::component::Component;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selected(pub u8);
