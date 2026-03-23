use std::sync::Arc;

use bevy_asset::{Asset, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use mcrs_core::{rl, ResourceKey};
use serde::Deserialize;

use super::dimension_type::DimensionType;
use crate::worldgen::chunk_generator::{ChunkGenerator, ProtoChunkGenerator};
use crate::ResourceLocation;

// ===========================================================================
// Well-known dimension keys
// ===========================================================================

pub const OVERWORLD: ResourceKey<DimensionDefinition, &'static str> =
    ResourceKey::new(rl!("minecraft:overworld"));
pub const THE_NETHER: ResourceKey<DimensionDefinition, &'static str> =
    ResourceKey::new(rl!("minecraft:the_nether"));
pub const THE_END: ResourceKey<DimensionDefinition, &'static str> =
    ResourceKey::new(rl!("minecraft:the_end"));

// ===========================================================================
// Runtime type
// ===========================================================================

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

// ===========================================================================
// Proto type (serde layer)
// ===========================================================================

#[derive(Deserialize)]
pub(crate) struct ProtoDimensionEntry {
    #[serde(rename = "type")]
    pub(crate) dimension_type: ResourceLocation<Arc<str>>,
    pub(crate) generator: ProtoChunkGenerator,
}

// ===========================================================================
// Resolve: Proto → Runtime
// ===========================================================================

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
