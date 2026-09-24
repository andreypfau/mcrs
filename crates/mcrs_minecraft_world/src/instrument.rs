use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct Instrument {
    pub sound_event: String,
    pub use_duration: f32,
    pub range: f32,
    pub description: serde_json::Value,
}

impl Asset for Instrument {}

impl VisitAssetDependencies for Instrument {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn deserialize_all_instruments() {
        mcrs_minecraft_worldgen_testing::parse_all::<super::Instrument>("minecraft/instrument");
    }
}
