use std::sync::Arc;

use bevy_asset::{Handle, LoadContext, UntypedAssetId};
use serde::Deserialize;

use mcrs_minecraft_worldgen::bevy::NoiseGeneratorSettingsAsset;

use super::flat::{FlatChunkGenerator, ProtoFlatChunkGenerator};
use crate::ResourceLocation;
use mcrs_minecraft_biome::source::ProtoBiomeSource;

// ===========================================================================
// Runtime types
// ===========================================================================

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ChunkGenerator {
    Noise(NoiseChunkGenerator),
    Flat(FlatChunkGenerator),
    Debug,
}

impl ChunkGenerator {
    pub(crate) fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        match self {
            ChunkGenerator::Noise(g) => visit(g.settings.id().untyped()),
            ChunkGenerator::Flat(_) | ChunkGenerator::Debug => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoiseChunkGenerator {
    // chisle: names until the preset is parsed with the registries in scope; the
    // server resolves them where it builds a dimension's generator.
    pub biome_source: ProtoBiomeSource,
    pub settings: Handle<NoiseGeneratorSettingsAsset>,
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
            ProtoChunkGenerator::Flat(f) => ChunkGenerator::Flat(f.resolve()),
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
            biome_source: self.biome_source,
            settings: ctx.load(settings_path),
        }
    }
}
