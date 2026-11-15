use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_resource::Id;

/// The stack of the parameter `tool`; no item is the empty stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tool<'a> {
    pub item: Option<Id<Item>>,
    pub enchantments: &'a [(Id<Enchantment>, u32)],
}

impl Tool<'_> {
    /// The level of `enchantment` on the tool; 0 without it (LOOT-CND-022).
    #[must_use]
    pub fn level(&self, enchantment: Id<Enchantment>) -> u32 {
        self.enchantments
            .iter()
            .find(|(id, _)| *id == enchantment)
            .map_or(0, |&(_, level)| level)
    }
}
