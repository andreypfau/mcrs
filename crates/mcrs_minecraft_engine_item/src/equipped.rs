use bevy_ecs::component::Component;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[component(immutable)]
pub struct Equipped;
