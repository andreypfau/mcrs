use crate::world::item::component::ItemComponents;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Component;
use mcrs_protocol::{Ident, ItemId, Slot};

pub mod component;
pub mod minecraft;

pub struct Item {
    pub id: ItemId,
    pub identifier: Ident<&'static str>,
    pub components: &'static ItemComponents,
}

impl From<&'static Item> for ItemId {
    fn from(item: &'static Item) -> Self {
        item.id
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct ItemStack {
    item_id: ItemId,
    count: u8,
}

impl ItemStack {
    pub fn item_id(&self) -> ItemId {
        self.item_id
    }

    pub fn count(&self) -> u8 {
        self.count
    }
}

impl From<ItemStack> for Slot {
    fn from(value: ItemStack) -> Self {
        Slot {
            id: value.item_id,
            count: value.count,
            components: Default::default(),
        }
    }
}

pub trait ItemCommands {
    fn spawn_item_stack<I>(&mut self, item_id: I, count: u8) -> Entity
    where
        I: Into<ItemId>;
}

impl ItemCommands for bevy_ecs::prelude::Commands<'_, '_> {
    fn spawn_item_stack<I>(&mut self, item_id: I, count: u8) -> Entity
    where
        I: Into<ItemId>,
    {
        let item_id = item_id.into();
        self.spawn(ItemStack { item_id, count }).id()
    }
}
