use std::sync::Arc;

use bevy_asset::{Asset, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::Deserialize;

use crate::ResourceLocation;
use crate::worldgen::chunk_generator::{ChunkGenerator, ProtoChunkGenerator};

/// A dimension definition: the name of a dimension type plus a chunk generator.
///
/// Produced by `WorldPresetLoader` as labeled sub-assets.
#[derive(Debug, Clone, TypePath)]
pub struct DimensionDefinition {
    pub dimension_type: ResourceLocation<Arc<str>>,
    pub generator: ChunkGenerator,
}

impl Asset for DimensionDefinition {}

impl VisitAssetDependencies for DimensionDefinition {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.generator.visit_dependencies(visit);
    }
}

#[derive(Deserialize)]
pub(crate) struct ProtoDimensionEntry {
    #[serde(rename = "type")]
    pub(crate) dimension_type: ResourceLocation<Arc<str>>,
    pub(crate) generator: ProtoChunkGenerator,
}

impl ProtoDimensionEntry {
    pub(crate) fn resolve(self, ctx: &mut LoadContext) -> DimensionDefinition {
        DimensionDefinition {
            dimension_type: self.dimension_type,
            generator: self.generator.resolve(ctx),
        }
    }
}
