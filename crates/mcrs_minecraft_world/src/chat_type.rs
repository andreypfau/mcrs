use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ChatType {
    pub chat: ChatDecoration,
    pub narration: ChatDecoration,
    #[serde(default)]
    pub overlay: Option<ChatDecoration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDecoration {
    pub translation_key: String,
    pub parameters: Vec<String>,
    #[serde(default)]
    pub style: Option<serde_json::Value>,
}

impl Asset for ChatType {}

impl VisitAssetDependencies for ChatType {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}
