use bevy_ecs::component::Component;
use mcrs_minecraft_inventory_click::Drag;

/// The drag state of the inventory menu of a player.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClickDrag(pub(crate) Drag);
