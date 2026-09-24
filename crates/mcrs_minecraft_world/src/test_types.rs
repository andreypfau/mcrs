use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct TestEnvironment {
    #[serde(rename = "type")]
    pub env_type: String,
    #[serde(default)]
    pub definitions: Vec<serde_json::Value>,
}

impl Asset for TestEnvironment {}

impl VisitAssetDependencies for TestEnvironment {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct TestInstance {
    #[serde(rename = "type")]
    pub test_type: String,
    pub function: String,
    pub max_ticks: u32,
    pub setup_ticks: u32,
    pub required: bool,
    pub environment: String,
    pub structure: String,
}

impl Asset for TestInstance {}

impl VisitAssetDependencies for TestInstance {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn deserialize_all_test_environments() {
        mcrs_minecraft_worldgen_testing::parse_all::<super::TestEnvironment>(
            "minecraft/test_environment",
        );
    }

    #[test]
    fn deserialize_all_test_instances() {
        mcrs_minecraft_worldgen_testing::parse_all::<super::TestInstance>(
            "minecraft/test_instance",
        );
    }
}
