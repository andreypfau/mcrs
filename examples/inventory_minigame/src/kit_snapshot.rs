use bevy_ecs::component::Component;
use mcrs_minecraft_inventory_file::PlayerItems;

/// The items that a player held when it entered the minigame. A player is in the minigame while
/// it has this component.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct KitSnapshot(pub PlayerItems);
