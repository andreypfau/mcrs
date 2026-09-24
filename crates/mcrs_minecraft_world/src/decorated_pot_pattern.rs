use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(deny_unknown_fields)]
pub struct DecoratedPotPattern {
    pub asset_id: String,
}

impl Asset for DecoratedPotPattern {}

impl VisitAssetDependencies for DecoratedPotPattern {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

