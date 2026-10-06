use crate::Text;
use crate::component::EquipmentSlotGroup;
use mcrs_minecraft_keys::{Enchantment, Item};
use mcrs_minecraft_registry::HolderSet;
use serde::{Deserialize, Serialize};

/// An enchantment's effects are the `mcrs_minecraft_enchantment` column beside it.
#[derive(Debug, Clone)]
pub struct EnchantmentData {
    pub description: Text,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<EquipmentSlotGroup>,
    pub supported_items: HolderSet<Item>,
    pub primary_items: Option<HolderSet<Item>>,
    pub weight: u32,
    pub max_level: u32,
    pub exclusive_set: Option<HolderSet<Enchantment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}
