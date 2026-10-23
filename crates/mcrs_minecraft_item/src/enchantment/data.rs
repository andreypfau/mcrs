use crate::Text;
use crate::component::EquipmentSlotGroup;
use crate::keys::Item;
use mcrs_minecraft_registries::{Refs, Resolve, Resolved};
use serde::{Deserialize, Serialize};

/// An enchantment's effects are the `mcrs_minecraft_enchantment` column beside it.
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
    pub exclusive_set: Option<M::Set<EnchantmentData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}
