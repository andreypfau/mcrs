use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Component;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use mcrs_minecraft_protocol::Slot;
use mcrs_minecraft_protocol::item::ComponentMap;
use mcrs_minecraft_registry::ItemId;
use std::sync::LazyLock;

pub mod component;
pub mod enchantment;
pub mod minecraft;
pub mod tags;
pub mod trim;

pub struct Item {
    pub id: ItemId,
    pub identifier: ResourceLocation<&'static str>,
    pub components: LazyLock<ComponentMap>,
}

impl TaggedRegistry for Item {
    const REGISTRY_PATH: &'static str = "item";
}

impl From<&'static Item> for ItemId {
    fn from(item: &'static Item) -> Self {
        item.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Component)]
pub struct ItemStack {
    item_id: ItemId,
    count: u8,
}

impl ItemStack {
    pub fn new(item_id: impl Into<ItemId>, count: u8) -> Self {
        Self {
            item_id: item_id.into(),
            count,
        }
    }

    pub fn item_id(&self) -> ItemId {
        self.item_id
    }

    pub fn count(&self) -> u8 {
        self.count
    }
}

/// ponytail: a stack carries no per-stack patch yet, so the wire form is the
/// bare prototype; diff `ComponentMap`s here once `ItemStack` holds one.
impl From<ItemStack> for Slot {
    fn from(value: ItemStack) -> Self {
        Slot {
            id: value.item_id,
            count: i32::from(value.count),
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
