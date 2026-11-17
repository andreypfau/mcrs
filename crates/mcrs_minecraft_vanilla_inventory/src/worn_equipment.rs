use bevy_ecs::component::Component;
use mcrs_minecraft_item_stack::ItemStack;

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct WornEquipment {
    pub mainhand: Option<ItemStack>,
    pub offhand: Option<ItemStack>,
    pub feet: Option<ItemStack>,
    pub legs: Option<ItemStack>,
    pub chest: Option<ItemStack>,
    pub head: Option<ItemStack>,
    pub body: Option<ItemStack>,
    pub saddle: Option<ItemStack>,
}
