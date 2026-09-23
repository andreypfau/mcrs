//! The per-step feature order and the per-biome sets, against the dump the
//! reference produced for the same three biome sources.

use bytes::Buf;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_feature::compile::{LoadedFeatures, build_feature_steps};
use mcrs_minecraft_worldgen_feature::proto::FeatureStepList;
use mcrs_minecraft_worldgen_testing as corpus;
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};
use serde::Deserialize;
use std::path::PathBuf;

const MAGIC: &[u8; 8] = b"MCFSTEP0";

struct Source {
    id: String,
    biomes: Vec<ResourceLocation>,
    steps: Vec<Vec<String>>,
    /// `[biome][step]`, the feature positions the biome carries.
    per_biome: Vec<Vec<Vec<usize>>>,
}

fn read_dump() -> Vec<Source> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vanilla/feature_steps.bin");
    let mut r = open_dump(&path, MAGIC);

    let sources = (0..r.get_u32_le())
        .map(|_| {
            let id = dump_string(&mut r);
            let biomes: Vec<ResourceLocation> = (0..r.get_u32_le())
                .map(|_| ResourceLocation::parse(&dump_string(&mut r)).unwrap())
                .collect();
            let steps: Vec<Vec<String>> = (0..r.get_u32_le())
                .map(|_| (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect())
                .collect();
            let per_biome = biomes
                .iter()
                .map(|_| {
                    steps
                        .iter()
                        .map(|step| {
                            let len = r.get_u32_le() as usize;
                            let bytes = r.copy_to_bytes(len);
                            assert_eq!(bytes.len(), step.len().div_ceil(8));
                            (0..step.len())
                                .filter(|index| bytes[index >> 3] & (1 << (index & 7)) != 0)
                                .collect()
                        })
                        .collect()
                })
                .collect();
            Source {
                id,
                biomes,
                steps,
                per_biome,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    sources
}

#[derive(Deserialize)]
struct BiomeGeneration {
    #[serde(default)]
    features: Vec<FeatureStepList>,
}

#[test]
fn the_step_order_and_the_per_biome_sets_equal_the_reference() {
    let registries = LoadedFeatures {
        features: corpus::registry("feature"),
        placed_features: corpus::registry("placed_feature"),
        ..Default::default()
    };

    let mut compared = 0;
    for source in read_dump() {
        let generation: Vec<BiomeGeneration> = source
            .biomes
            .iter()
            .map(|id| corpus::read::<BiomeGeneration>("biome", id))
            .collect();
        let biomes: Vec<&[FeatureStepList]> =
            generation.iter().map(|biome| &biome.features[..]).collect();

        let built = build_feature_steps(&biomes, &registries)
            .unwrap_or_else(|e| panic!("{}: {e}", source.id));

        let ids: Vec<Vec<String>> = built
            .steps
            .iter()
            .map(|step| {
                step.iter()
                    .map(|feature| {
                        feature
                            .id
                            .as_ref()
                            .unwrap_or_else(|| {
                                panic!("{}: the corpus names every feature by id", source.id)
                            })
                            .to_string()
                    })
                    .collect()
            })
            .collect();
        assert_eq!(ids, source.steps, "{}: step order", source.id);

        for (index, biome) in source.biomes.iter().enumerate() {
            let ours: Vec<Vec<usize>> = built.per_biome[index]
                .iter()
                .map(|bits| bits.ones().collect())
                .collect();
            assert_eq!(
                ours, source.per_biome[index],
                "{}: {biome} carries other features than the reference says",
                source.id
            );
        }
        compared += 1;
    }
    assert_eq!(compared, 3, "every source in the dump is compared");
}
