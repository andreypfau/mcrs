use std::sync::Arc;

use bevy_asset::{Asset, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::Deserialize;

use crate::ResourceLocation;
use crate::worldgen::chunk_generator::{ChunkGenerator, ProtoChunkGenerator};
use mcrs_minecraft_dimension::dimension_type::DimensionType;

/// A dimension definition: a dimension type reference plus a chunk generator.
///
/// Produced by `WorldPresetLoader` as labeled sub-assets.
#[derive(Debug, Clone, TypePath)]
pub struct DimensionDefinition {
    pub dimension_type: Handle<DimensionType>,
    pub generator: ChunkGenerator,
}

impl Asset for DimensionDefinition {}

impl VisitAssetDependencies for DimensionDefinition {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.dimension_type.id().untyped());
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
        let dimension_type = DimensionType::load(ctx, &self.dimension_type);
        let generator = self.generator.resolve(ctx);
        DimensionDefinition {
            dimension_type,
            generator,
        }
    }
}
