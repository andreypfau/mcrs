use std::sync::Arc;

use bevy_asset::{Handle, LoadContext, UntypedAssetId};
use serde::Deserialize;

use super::structure_set::StructureSet;
use crate::biome::Biome;
use crate::ResourceLocation;

// ===========================================================================
// Runtime types
// ===========================================================================

#[derive(Debug, Clone)]
pub struct FlatChunkGenerator {
    pub settings: FlatLevelGeneratorSettings,
}

impl FlatChunkGenerator {
    pub(crate) fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.settings.visit_dependencies(visit);
    }
}

#[derive(Debug, Clone)]
pub struct FlatLevelGeneratorSettings {
    pub biome: Handle<Biome>,
    pub features: bool,
    pub lakes: bool,
    pub layers: Vec<FlatLayerInfo>,
    pub structure_overrides: Vec<Handle<StructureSet>>,
}

impl FlatLevelGeneratorSettings {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.biome.id().untyped());
        for s in &self.structure_overrides {
            visit(s.id().untyped());
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlatLayerInfo {
    pub block: ResourceLocation<Arc<str>>,
    pub height: u32,
}

// ===========================================================================
// Proto types (serde layer)
// ===========================================================================

#[derive(Deserialize)]
pub(crate) struct ProtoFlatChunkGenerator {
    pub(crate) settings: ProtoFlatLevelGeneratorSettings,
}

#[derive(Deserialize)]
pub(crate) struct ProtoFlatLevelGeneratorSettings {
    #[serde(default = "default_plains")]
    pub(crate) biome: ResourceLocation<Arc<str>>,
    #[serde(default)]
    pub(crate) features: bool,
    #[serde(default)]
    pub(crate) lakes: bool,
    pub(crate) layers: Vec<ProtoFlatLayerInfo>,
    #[serde(default)]
    pub(crate) structure_overrides: Vec<ResourceLocation<Arc<str>>>,
}

fn default_plains() -> ResourceLocation<Arc<str>> {
    ResourceLocation::minecraft("plains")
}

#[derive(Deserialize)]
pub(crate) struct ProtoFlatLayerInfo {
    pub(crate) block: ResourceLocation<Arc<str>>,
    pub(crate) height: u32,
}

// ===========================================================================
// Resolve: Proto → Runtime
// ===========================================================================

impl ProtoFlatChunkGenerator {
    pub(crate) fn resolve(self, ctx: &mut LoadContext) -> FlatChunkGenerator {
        FlatChunkGenerator {
            settings: self.settings.resolve(ctx),
        }
    }
}

impl ProtoFlatLevelGeneratorSettings {
    fn resolve(self, ctx: &mut LoadContext) -> FlatLevelGeneratorSettings {
        FlatLevelGeneratorSettings {
            biome: Biome::load(ctx, &self.biome),
            features: self.features,
            lakes: self.lakes,
            layers: self
                .layers
                .into_iter()
                .map(|l| FlatLayerInfo {
                    block: l.block,
                    height: l.height,
                })
                .collect(),
            structure_overrides: self
                .structure_overrides
                .into_iter()
                .map(|loc| StructureSet::load(ctx, &loc))
                .collect(),
        }
    }
}
