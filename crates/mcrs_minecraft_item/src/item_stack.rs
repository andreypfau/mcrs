use crate::keys::Item;
use bevy_ecs::prelude::Component;
use mcrs_minecraft_registry::Id;

/// On a stack entity only a stack transaction assigns the fields; the
/// constructor serves inline prototype-only values such as mob equipment.
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
