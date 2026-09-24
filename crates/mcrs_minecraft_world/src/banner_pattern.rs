use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct BannerPattern {
    pub asset_id: String,
    pub translation_key: String,
}

impl Asset for BannerPattern {}

impl VisitAssetDependencies for BannerPattern {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn deserialize_all_banner_patterns() {
        mcrs_minecraft_worldgen_testing::parse_all::<super::BannerPattern>(
            "minecraft/banner_pattern",
        );
    }
}
