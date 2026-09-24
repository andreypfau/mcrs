use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct PaintingVariant {
    pub asset_id: String,
    #[serde(default)]
    pub title: Option<serde_json::Value>,
    #[serde(default)]
    pub author: Option<serde_json::Value>,
    pub width: u32,
    pub height: u32,
}

impl Asset for PaintingVariant {}

impl VisitAssetDependencies for PaintingVariant {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn deserialize_all_painting_variants() {
        mcrs_minecraft_worldgen_testing::parse_all::<super::PaintingVariant>(
            "minecraft/painting_variant",
        );
    }
}
