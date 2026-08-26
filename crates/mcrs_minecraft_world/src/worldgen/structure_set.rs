use std::sync::Arc;

use bevy_asset::{Asset, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;

use crate::ResourceLocation;

/// Stub for structure set definitions.
///
/// Full deserialization will be added when the structure placement pipeline
/// consumes these definitions.
#[derive(Debug, Clone, Default, TypePath)]
pub struct StructureSet;

impl StructureSet {
    pub fn load(
        ctx: &mut LoadContext<'_>,
        loc: &ResourceLocation<Arc<str>>,
    ) -> Handle<StructureSet> {
        ctx.load(format!(
            "{}/worldgen/structure_set/{}.json",
            loc.namespace(),
            loc.path()
        ))
    }
}

impl Asset for StructureSet {}

impl VisitAssetDependencies for StructureSet {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}
