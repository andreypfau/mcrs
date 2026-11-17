use bevy_ecs::component::Component;
use mcrs_minecraft_inventory_click::Drag;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClickDrag(pub(crate) Drag);
