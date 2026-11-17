use bevy_ecs::component::Component;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SentSelected(pub(crate) u8);
