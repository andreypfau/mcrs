use std::sync::Arc;

use bevy_asset::{Handle, LoadContext, UntypedAssetId};
use serde::Deserialize;

use super::climate::ClimateParameters;
use super::Biome;
use crate::ResourceLocation;

// ===========================================================================
// Runtime types
// ===========================================================================

#[derive(Debug, Clone)]
pub enum BiomeSource {
    MultiNoise(MultiNoiseBiomeSource),
    TheEnd,
    Fixed {
        biome: Handle<Biome>,
    },
    Checkerboard {
        biomes: Vec<Handle<Biome>>,
        scale: u32,
    },
}

impl BiomeSource {
    pub(crate) fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        match self {
            BiomeSource::MultiNoise(src) => {
                if let Some(biomes) = &src.biomes {
                    for entry in biomes {
                        visit(entry.biome.id().untyped());
                    }
                }
            }
            BiomeSource::Fixed { biome } => visit(biome.id().untyped()),
            BiomeSource::Checkerboard { biomes, .. } => {
                for b in biomes {
                    visit(b.id().untyped());
                }
            }
            BiomeSource::TheEnd => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct MultiNoiseBiomeSource {
    pub preset: Option<ResourceLocation<Arc<str>>>,
    pub biomes: Option<Vec<MultiNoiseBiomeEntry>>,
}

#[derive(Debug, Clone)]
pub struct MultiNoiseBiomeEntry {
    pub parameters: ClimateParameters,
    pub biome: Handle<Biome>,
}

// ===========================================================================
// Proto types (serde layer)
// ===========================================================================

#[derive(Deserialize)]
#[serde(tag = "type")]
pub(crate) enum ProtoBiomeSource {
    #[serde(rename = "minecraft:multi_noise")]
    MultiNoise(ProtoMultiNoiseBiomeSource),
    #[serde(rename = "minecraft:the_end")]
    TheEnd {},
    #[serde(rename = "minecraft:fixed")]
    Fixed { biome: ResourceLocation<Arc<str>> },
    #[serde(rename = "minecraft:checkerboard")]
    Checkerboard {
        biomes: Vec<ResourceLocation<Arc<str>>>,
        #[serde(default = "default_scale")]
        scale: u32,
    },
}

fn default_scale() -> u32 {
    2
}

#[derive(Deserialize)]
pub(crate) struct ProtoMultiNoiseBiomeSource {
    pub(crate) preset: Option<ResourceLocation<Arc<str>>>,
    pub(crate) biomes: Option<Vec<ProtoMultiNoiseBiomeEntry>>,
}

#[derive(Deserialize)]
pub(crate) struct ProtoMultiNoiseBiomeEntry {
    pub(crate) parameters: ClimateParameters,
    pub(crate) biome: ResourceLocation<Arc<str>>,
}

// ===========================================================================
// Resolve: Proto → Runtime
// ===========================================================================

impl ProtoBiomeSource {
    pub(crate) fn resolve(self, ctx: &mut LoadContext) -> BiomeSource {
        match self {
            ProtoBiomeSource::MultiNoise(src) => BiomeSource::MultiNoise(src.resolve(ctx)),
            ProtoBiomeSource::TheEnd {} => BiomeSource::TheEnd,
            ProtoBiomeSource::Fixed { biome } => BiomeSource::Fixed {
                biome: Biome::load(ctx, &biome),
            },
            ProtoBiomeSource::Checkerboard { biomes, scale } => BiomeSource::Checkerboard {
                biomes: biomes
                    .into_iter()
                    .map(|loc| Biome::load(ctx, &loc))
                    .collect(),
                scale,
            },
        }
    }
}

impl ProtoMultiNoiseBiomeSource {
    fn resolve(self, ctx: &mut LoadContext) -> MultiNoiseBiomeSource {
        MultiNoiseBiomeSource {
            preset: self.preset,
            biomes: self.biomes.map(|entries| {
                entries
                    .into_iter()
                    .map(|e| MultiNoiseBiomeEntry {
                        parameters: e.parameters,
                        biome: Biome::load(ctx, &e.biome),
                    })
                    .collect()
            }),
        }
    }
}
