use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct DamageType {
    pub exhaustion: f32,
    pub message_id: String,
    pub scaling: String,
    #[serde(default)]
    pub death_message_type: Option<String>,
    #[serde(default)]
    pub effects: Option<String>,
}

impl Asset for DamageType {}

impl VisitAssetDependencies for DamageType {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}
