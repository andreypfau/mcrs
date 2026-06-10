use std::sync::Arc;

use bevy_asset::{Handle, LoadContext, UntypedAssetId};
use serde::Deserialize;

use super::flat::{FlatChunkGenerator, ProtoFlatChunkGenerator};
use super::noise_settings::NoiseGeneratorSettings;
use crate::biome::source::{BiomeSource, ProtoBiomeSource};
use crate::ResourceLocation;

// ===========================================================================
// Runtime types
// ===========================================================================

#[derive(Debug, Clone)]
pub enum ChunkGenerator {
    Noise(NoiseChunkGenerator),
    Flat(FlatChunkGenerator),
    Debug,
}

impl ChunkGenerator {
    pub(crate) fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        match self {
            ChunkGenerator::Noise(g) => g.visit_dependencies(visit),
            ChunkGenerator::Flat(g) => g.visit_dependencies(visit),
            ChunkGenerator::Debug => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoiseChunkGenerator {
    pub biome_source: BiomeSource,
    pub settings: Handle<NoiseGeneratorSettings>,
}

impl NoiseChunkGenerator {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.settings.id().untyped());
        self.biome_source.visit_dependencies(visit);
    }
}

// ===========================================================================
// Proto types (serde layer)
// ===========================================================================

#[derive(Deserialize)]
#[serde(tag = "type")]
pub(crate) enum ProtoChunkGenerator {
    #[serde(rename = "minecraft:noise")]
    Noise(ProtoNoiseChunkGenerator),
    #[serde(rename = "minecraft:flat")]
    Flat(ProtoFlatChunkGenerator),
    #[serde(rename = "minecraft:debug")]
    Debug,
}

#[derive(Deserialize)]
pub(crate) struct ProtoNoiseChunkGenerator {
    pub(crate) biome_source: ProtoBiomeSource,
    pub(crate) settings: ResourceLocation<Arc<str>>,
}

// ===========================================================================
// Resolve: Proto → Runtime
// ===========================================================================

impl ProtoChunkGenerator {
    pub(crate) fn resolve(self, ctx: &mut LoadContext) -> ChunkGenerator {
        match self {
            ProtoChunkGenerator::Noise(n) => ChunkGenerator::Noise(n.resolve(ctx)),
            ProtoChunkGenerator::Flat(f) => ChunkGenerator::Flat(f.resolve(ctx)),
            ProtoChunkGenerator::Debug => ChunkGenerator::Debug,
        }
    }
}

impl ProtoNoiseChunkGenerator {
    fn resolve(self, ctx: &mut LoadContext) -> NoiseChunkGenerator {
        let settings_path = format!(
            "{}/worldgen/noise_settings/{}.json",
            self.settings.namespace(),
            self.settings.path()
        );
        NoiseChunkGenerator {
            biome_source: self.biome_source.resolve(ctx),
            settings: ctx.load(settings_path),
        }
    }
}
