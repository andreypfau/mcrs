use mcrs_minecraft_item::Text;
use mcrs_minecraft_item::component::EquipmentSlotGroup;
use mcrs_minecraft_item::enchantment::{EnchantmentCost, EnchantmentData};
use mcrs_minecraft_keys::{Enchantment, Item};
use mcrs_minecraft_registry::HolderSet;
use serde::{Deserialize, Serialize};

use crate::effects::EnchantmentEffects;

/// An enchantment file as a data pack writes it, stored as its registry value
/// and its effects column.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentFile {
    pub description: Text,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<EquipmentSlotGroup>,
    pub supported_items: HolderSet<Item>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_items: Option<HolderSet<Item>>,
    pub weight: u32,
    pub max_level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_set: Option<HolderSet<Enchantment>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<EnchantmentEffects>,
}

impl EnchantmentFile {
    pub fn split(&self) -> (EnchantmentData, Option<EnchantmentEffects>) {
        let data = EnchantmentData {
            description: self.description.clone(),
            min_cost: self.min_cost.clone(),
            max_cost: self.max_cost.clone(),
            anvil_cost: self.anvil_cost,
            slots: self.slots.clone(),
            supported_items: self.supported_items.clone(),
            primary_items: self.primary_items.clone(),
            weight: self.weight,
            max_level: self.max_level,
            exclusive_set: self.exclusive_set.clone(),
        };
        (data, self.effects.clone())
    }

    pub fn join((data, effects): (&EnchantmentData, &Option<EnchantmentEffects>)) -> Self {
        EnchantmentFile {
            description: data.description.clone(),
            min_cost: data.min_cost.clone(),
            max_cost: data.max_cost.clone(),
            anvil_cost: data.anvil_cost,
            slots: data.slots.clone(),
            supported_items: data.supported_items.clone(),
            primary_items: data.primary_items.clone(),
            weight: data.weight,
            max_level: data.max_level,
            exclusive_set: data.exclusive_set.clone(),
            effects: effects.clone(),
        }
    }
}
