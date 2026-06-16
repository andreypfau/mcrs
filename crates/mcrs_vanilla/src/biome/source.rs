use std::sync::Arc;

use bevy_asset::{Handle, LoadContext, UntypedAssetId};
use serde::Deserialize;

use super::climate::ClimateParameters;
use super::Biome;
use crate::ResourceLocation;

// ===========================================================================
// Beta biome lookup — enum, cascade, table, ocean mapping
// ===========================================================================

/// Discriminant order is the contract for the `biomes` list order in the
/// `mcrs:beta` biome_source JSON. The JSON entry at index N must correspond to
/// the BetaLandBiome variant whose discriminant equals N.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BetaLandBiome {
    IceDesert = 0,
    Tundra = 1,
    Savanna = 2,
    Desert = 3,
    Swampland = 4,
    Taiga = 5,
    Shrubland = 6,
    Forest = 7,
    Plains = 8,
    SeasonalForest = 9,
    Rainforest = 10,
}

pub fn beta_get_biome(temp: f32, rain: f32) -> BetaLandBiome {
    let rain = rain * temp;
    if temp < 0.1 {
        return BetaLandBiome::IceDesert;
    }
    if rain < 0.2 {
        if temp < 0.5 {
            return BetaLandBiome::Tundra;
        }
        if temp < 0.95 {
            return BetaLandBiome::Savanna;
        }
        return BetaLandBiome::Desert;
    }
    if rain > 0.5 && temp < 0.7 {
        return BetaLandBiome::Swampland;
    }
    if temp < 0.5 {
        return BetaLandBiome::Taiga;
    }
    if temp < 0.97 {
        if rain < 0.35 {
            return BetaLandBiome::Shrubland;
        }
        return BetaLandBiome::Forest;
    }
    if rain < 0.45 {
        return BetaLandBiome::Plains;
    }
    if rain < 0.9 {
        return BetaLandBiome::SeasonalForest;
    }
    BetaLandBiome::Rainforest
}

pub fn build_beta_lookup_table() -> [[BetaLandBiome; 64]; 64] {
    let mut table = [[BetaLandBiome::IceDesert; 64]; 64];
    for i in 0..64usize {
        for j in 0..64usize {
            table[i][j] = beta_get_biome(i as f32 / 63.0, j as f32 / 63.0);
        }
    }
    table
}

/// Resolve a land biome via the precomputed 64x64 quantized lookup, mirroring
/// BiomeBase.getBiomeFromLookup: i=(int)(temp*63), j=(int)(rain*63).
pub fn beta_biome_from_climate(
    table: &[[BetaLandBiome; 64]; 64],
    temp: f32,
    rain: f32,
) -> BetaLandBiome {
    let ti = ((temp * 63.0) as usize).min(63);
    let ri = ((rain * 63.0) as usize).min(63);
    table[ti][ri]
}

/// Returns the index into the `ocean_biomes` array for a given land bucket.
/// Ocean array order: [FrozenOcean=0, Ocean=1, WarmOcean=2, LukewarmOcean=3, ColdOcean=4]
pub fn ocean_biome_for(bucket: BetaLandBiome) -> usize {
    match bucket {
        BetaLandBiome::IceDesert => 0,      // FrozenOcean
        BetaLandBiome::Tundra => 0,         // FrozenOcean
        BetaLandBiome::Taiga => 0,          // FrozenOcean
        BetaLandBiome::Swampland => 4,      // ColdOcean
        BetaLandBiome::SeasonalForest => 3, // LukewarmOcean
        BetaLandBiome::Rainforest => 2,     // WarmOcean
        _ => 1,                             // Ocean (desert, savanna, shrubland, forest, plains)
    }
}

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
    Beta {
        // Array indexed by BetaLandBiome discriminant (0..=10, 11 buckets).
        // JSON biomes list order must match BetaLandBiome discriminant values.
        land_biomes: [Handle<Biome>; 11],
        ocean_biomes: [Handle<Biome>; 5],
        // Resource locations parallel to the handle arrays. Biome palette fill
        // resolves network IDs by location, not AssetId: chunk generation runs in
        // a per-dim sub-app whose AssetServer assigns different AssetIds than the
        // host that built the biome RegistrySnapshot, so AssetId lookups collide.
        land_biome_ids: [ResourceLocation<Arc<str>>; 11],
        ocean_biome_ids: [ResourceLocation<Arc<str>>; 5],
        lookup: Box<[[BetaLandBiome; 64]; 64]>,
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
            BiomeSource::Beta {
                land_biomes,
                ocean_biomes,
                ..
            } => {
                for b in land_biomes {
                    visit(b.id().untyped());
                }
                for b in ocean_biomes {
                    visit(b.id().untyped());
                }
            }
            BiomeSource::TheEnd => {}
        }
    }

    pub fn beta_biome_id(&self, temp: f32, rain: f32, is_ocean: bool) -> bevy_asset::AssetId<Biome> {
        match self {
            BiomeSource::Beta {
                land_biomes,
                ocean_biomes,
                lookup,
                ..
            } => {
                let ti = (temp * 63.0).clamp(0.0, 63.0) as usize;
                let ri = (rain * 63.0).clamp(0.0, 63.0) as usize;
                let bucket = lookup[ti][ri];
                if is_ocean {
                    ocean_biomes[ocean_biome_for(bucket)].id()
                } else {
                    land_biomes[bucket as usize].id()
                }
            }
            _ => panic!("beta_biome_id called on non-Beta BiomeSource"),
        }
    }

    /// Resolve the biome's resource location from Beta climate. Stable across
    /// AssetServers; use with [`RegistrySnapshot::by_location`] to get a network ID.
    pub fn beta_biome_location(&self, temp: f32, rain: f32, is_ocean: bool) -> &ResourceLocation<Arc<str>> {
        match self {
            BiomeSource::Beta {
                land_biome_ids,
                ocean_biome_ids,
                lookup,
                ..
            } => {
                let ti = (temp * 63.0).clamp(0.0, 63.0) as usize;
                let ri = (rain * 63.0).clamp(0.0, 63.0) as usize;
                let bucket = lookup[ti][ri];
                if is_ocean {
                    &ocean_biome_ids[ocean_biome_for(bucket)]
                } else {
                    &land_biome_ids[bucket as usize]
                }
            }
            _ => panic!("beta_biome_location called on non-Beta BiomeSource"),
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
    #[serde(rename = "mcrs:beta")]
    Beta {
        biomes: Vec<ResourceLocation<Arc<str>>>,
        ocean_biomes: Vec<ResourceLocation<Arc<str>>>,
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
            ProtoBiomeSource::Beta {
                biomes,
                ocean_biomes,
            } => {
                let land_biome_ids: [ResourceLocation<Arc<str>>; 11] = biomes
                    .clone()
                    .try_into()
                    .expect("mcrs:beta biome_source requires exactly 11 land biomes");
                let ocean_biome_ids: [ResourceLocation<Arc<str>>; 5] = ocean_biomes
                    .clone()
                    .try_into()
                    .expect("mcrs:beta biome_source requires exactly 5 ocean biomes");
                let land_handles: Vec<Handle<Biome>> =
                    biomes.into_iter().map(|l| Biome::load(ctx, &l)).collect();
                let ocean_handles: Vec<Handle<Biome>> = ocean_biomes
                    .into_iter()
                    .map(|l| Biome::load(ctx, &l))
                    .collect();
                BiomeSource::Beta {
                    land_biomes: land_handles
                        .try_into()
                        .expect("mcrs:beta biome_source requires exactly 11 land biomes"),
                    ocean_biomes: ocean_handles
                        .try_into()
                        .expect("mcrs:beta biome_source requires exactly 5 ocean biomes"),
                    land_biome_ids,
                    ocean_biome_ids,
                    lookup: Box::new(build_beta_lookup_table()),
                }
            }
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

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn beta_biome_all_land_reachable() {
        let table = build_beta_lookup_table();
        let mut seen: HashSet<u8> = HashSet::new();
        for row in &table {
            for biome in row {
                seen.insert(*biome as u8);
            }
        }
        assert_eq!(
            seen.len(),
            11,
            "Expected all 11 land buckets to be reachable, got {:?}",
            seen
        );
        // Verify each specific bucket is present
        for expected in 0u8..=10 {
            assert!(
                seen.contains(&expected),
                "Bucket with discriminant {} is not reachable from the 64x64 table",
                expected
            );
        }
    }

    #[test]
    fn beta_biome_lookup_table() {
        // temp < 0.1 → IceDesert
        assert_eq!(beta_get_biome(0.05, 0.5), BetaLandBiome::IceDesert);

        // temp=0.97, rain_adjusted = rain*temp; test Savanna: temp in [0.5,0.95), rain*temp < 0.2
        // temp=0.6, rain=0.1 → rain*temp=0.06 < 0.2, temp >= 0.5, temp < 0.95 → Savanna
        assert_eq!(beta_get_biome(0.6, 0.1), BetaLandBiome::Savanna);

        // Desert: temp >= 0.95, rain*temp < 0.2
        // temp=0.96, rain=0.1 → rain*temp=0.096 < 0.2, temp >= 0.95 → Desert
        assert_eq!(beta_get_biome(0.96, 0.1), BetaLandBiome::Desert);

        // SeasonalForest: temp >= 0.97, rain*temp in [0.45, 0.9)
        // temp=0.98, rain=0.5 → rain*temp=0.49 ≥ 0.45 and < 0.9 → SeasonalForest
        assert_eq!(beta_get_biome(0.98, 0.5), BetaLandBiome::SeasonalForest);

        // Rainforest: temp >= 0.97, rain*temp >= 0.9
        // temp=0.98, rain=0.95 → rain*temp=0.931 ≥ 0.9 → Rainforest
        assert_eq!(beta_get_biome(0.98, 0.95), BetaLandBiome::Rainforest);

        // Tundra: temp < 0.5, rain*temp < 0.2 (and temp >= 0.1)
        // temp=0.3, rain=0.5 → rain*temp=0.15 < 0.2, temp < 0.5 → Tundra
        assert_eq!(beta_get_biome(0.3, 0.5), BetaLandBiome::Tundra);

        // Swampland: rain*temp > 0.5, temp < 0.7
        // temp=0.6, rain=0.9 → rain*temp=0.54 > 0.5, temp < 0.7 → Swampland
        assert_eq!(beta_get_biome(0.6, 0.9), BetaLandBiome::Swampland);

        // Taiga: temp >= 0.5, rain*temp >= 0.2, rain*temp <= 0.5 (or temp < 0.7), temp < 0.5 fails → need temp in [0.5, 0.7) and rain not swampland
        // Actually Taiga: temp < 0.5 — wait let's re-check: after swampland check, if temp < 0.5 → Taiga
        // temp=0.4, rain=0.9 → rain*temp=0.36 >= 0.2, rain*temp <= 0.5 (0.36 not > 0.5), temp < 0.5 → Taiga
        assert_eq!(beta_get_biome(0.4, 0.9), BetaLandBiome::Taiga);

        // Shrubland: temp >= 0.5, temp < 0.97, rain*temp < 0.35
        // temp=0.7, rain=0.4 → rain*temp=0.28 < 0.35, temp < 0.97 → Shrubland
        assert_eq!(beta_get_biome(0.7, 0.4), BetaLandBiome::Shrubland);

        // Forest: temp >= 0.5, temp < 0.97, rain*temp >= 0.35
        // temp=0.7, rain=0.6 → rain*temp=0.42 >= 0.35, rain*temp <= 0.5 (not swampland since 0.42 ≤ 0.5), temp < 0.97 → Forest
        assert_eq!(beta_get_biome(0.7, 0.6), BetaLandBiome::Forest);

        // Plains: temp >= 0.97, rain*temp < 0.45
        // temp=0.98, rain=0.4 → rain*temp=0.392 < 0.45 → Plains
        assert_eq!(beta_get_biome(0.98, 0.4), BetaLandBiome::Plains);

        // Confirm rain is multiplied by temp before comparisons:
        // temp=0.3, rain=0.8 → rain*temp=0.24 >= 0.2, rain*temp <= 0.5 (no swampland), temp < 0.5 → Taiga
        // Without multiplication: rain=0.8 > 0.5 and temp=0.3 < 0.7 → Swampland (wrong)
        assert_eq!(beta_get_biome(0.3, 0.8), BetaLandBiome::Taiga);
    }

    #[test]
    fn ocean_biome_mapping() {
        assert_eq!(ocean_biome_for(BetaLandBiome::IceDesert), 0);
        assert_eq!(ocean_biome_for(BetaLandBiome::Tundra), 0);
        assert_eq!(ocean_biome_for(BetaLandBiome::Taiga), 0);
        assert_eq!(ocean_biome_for(BetaLandBiome::Swampland), 4);
        assert_eq!(ocean_biome_for(BetaLandBiome::SeasonalForest), 3);
        assert_eq!(ocean_biome_for(BetaLandBiome::Rainforest), 2);
        assert_eq!(ocean_biome_for(BetaLandBiome::Desert), 1);
        assert_eq!(ocean_biome_for(BetaLandBiome::Plains), 1);
        assert_eq!(ocean_biome_for(BetaLandBiome::Forest), 1);
    }
}
