use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct EnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: String,
    pub weight: u32,
    pub max_level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}

impl Asset for EnchantmentData {}

impl VisitAssetDependencies for EnchantmentData {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}
