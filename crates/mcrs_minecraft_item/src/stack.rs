use bevy_ecs::prelude::Component;
use mcrs_minecraft_registry::ItemId;

/// On a stack entity only a stack transaction assigns the fields; the
/// constructor serves inline prototype-only values such as mob equipment.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u8,
}

impl ItemStack {
    pub fn new(item: ItemId, count: u8) -> Self {
        Self { item, count }
    }

    pub fn item(&self) -> ItemId {
        self.item
    }

    pub fn count(&self) -> u8 {
        self.count
    }
}

#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackRevision(pub u32);
