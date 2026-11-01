use mcrs_minecraft_item::Text;
use mcrs_minecraft_item::component::EquipmentSlotGroup;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item::item_keys::Item;
use mcrs_minecraft_registries::{Refs, Resolve, Resolved};
use serde::{Deserialize, Serialize};

/// An enchantment's definition; its effects are the column [`EnchantmentFile`](crate::file::EnchantmentFile)
/// keeps beside it.
#[derive(Debug, Clone, Resolve)]
pub struct EnchantmentData<M: Refs = Resolved> {
    pub description: Text<M>,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<EquipmentSlotGroup>,
    pub supported_items: M::Set<Item>,
    pub primary_items: Option<M::Set<Item>>,
    pub weight: u32,
    pub max_level: u32,
    pub exclusive_set: Option<M::Set<Enchantment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}
