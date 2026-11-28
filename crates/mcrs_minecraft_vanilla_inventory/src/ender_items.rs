use bevy_ecs::component::Component;
use mcrs_minecraft_item_stack::SlotStack;

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct EnderItems(pub Vec<SlotStack>);
