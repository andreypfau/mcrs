use bevy_ecs::prelude::Component;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_resource::Id;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub item: Id<Item>,
    pub count: u8,
}

impl ItemStack {
    pub fn new(item: Id<Item>, count: u8) -> Self {
        Self { item, count }
    }
}

#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackRevision(pub u32);
