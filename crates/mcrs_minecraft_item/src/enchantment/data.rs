use crate::Item;
use crate::Text;
use crate::component::EquipmentSlotGroup;
use crate::enchantment::effects::EnchantmentEffects;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_registry::EntrySet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentData {
    pub description: Text,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<EquipmentSlotGroup>,
    pub supported_items: EntrySet<Item>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_items: Option<EntrySet<Item>>,
    pub weight: u32,
    pub max_level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_set: Option<EntrySet<EnchantmentData>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<EnchantmentEffects>,
}

impl RegistryKey for EnchantmentData {
    const KEY: mcrs_minecraft_core::ResourceLocation<&'static str> = rl!("minecraft:enchantment");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}
