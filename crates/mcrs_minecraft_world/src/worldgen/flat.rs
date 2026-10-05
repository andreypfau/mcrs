use std::sync::Arc;

use serde::Deserialize;

use crate::ResourceLocation;

// ===========================================================================
// Runtime types
// ===========================================================================

#[derive(Debug, Clone)]
pub struct FlatChunkGenerator {
    pub settings: FlatLevelGeneratorSettings,
}

#[derive(Debug, Clone)]
pub struct FlatLevelGeneratorSettings {
    pub biome: ResourceLocation<Arc<str>>,
    pub features: bool,
    pub lakes: bool,
    pub layers: Vec<FlatLayerInfo>,
    pub structure_overrides: Vec<ResourceLocation<Arc<str>>>,
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
    pub(crate) fn resolve(self) -> FlatChunkGenerator {
        FlatChunkGenerator {
            settings: self.settings.resolve(),
        }
    }
}

impl ProtoFlatLevelGeneratorSettings {
    fn resolve(self) -> FlatLevelGeneratorSettings {
        FlatLevelGeneratorSettings {
            biome: self.biome,
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
            structure_overrides: self.structure_overrides,
        }
    }
}
