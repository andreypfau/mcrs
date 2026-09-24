use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::overworld_preset::{nether_parameter_list, overworld_parameter_list};
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_feature::compile::FeatureSteps;
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;
use std::collections::BTreeMap;

/// `TheEndBiomeSource` lists its five biomes in this order, and that order is
/// the input of the sort.
const END_BIOMES: [&str; 5] = [
    "minecraft:the_end",
    "minecraft:end_highlands",
    "minecraft:end_midlands",
    "minecraft:small_end_islands",
    "minecraft:end_barrens",
];

/// One dimension's sorted feature tables: what [`FeatureProgram::build`]
/// resolves into numbers.
#[derive(Clone)]
pub struct FeatureTables {
    pub features: FeatureSteps,
    /// The biome source's own order, which decides the sort.
    pub biome_order: Vec<ResourceLocation>,
    /// What each biome's climate says about freezing. Only the parsed biome
    /// carries it, and the frozen registry the feature program builds against
    /// holds NBT, so it is read here and carried rather than resolved there.
    pub climate: BTreeMap<ResourceLocation, BiomeClimate>,
}

/// The biomes a source can answer with, in the source's own order and without
/// repeats — the input the feature order is defined against.
pub fn possible_biomes(
    source: &BiomeSource,
    named: impl Fn(&bevy_asset::Handle<Biome>) -> Option<ResourceLocation>,
) -> Vec<ResourceLocation> {
    let listed: Vec<ResourceLocation> = match source {
        BiomeSource::MultiNoise(multi) => match (&multi.biomes, &multi.preset) {
            (Some(entries), _) => entries.iter().map(|entry| entry.location.clone()).collect(),
            (None, Some(preset)) => match preset.as_str() {
                "minecraft:overworld" => preset_biomes(overworld_parameter_list()),
                "minecraft:nether" => preset_biomes(nether_parameter_list()),
                other => panic!("no biome list for the multi-noise preset {other}"),
            },
            (None, None) => panic!("a multi-noise source names neither biomes nor a preset"),
        },
        BiomeSource::TheEnd => END_BIOMES
            .iter()
            .filter_map(|id| ResourceLocation::parse(id).ok())
            .collect(),
        BiomeSource::Fixed { biome_id, .. } => vec![biome_id.clone()],
        BiomeSource::Checkerboard { biomes, .. } => biomes.iter().filter_map(named).collect(),
        BiomeSource::Beta { land_biome_ids, .. } => land_biome_ids.to_vec(),
    };

    let mut seen = std::collections::HashSet::new();
    listed
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

fn preset_biomes(
    list: &mcrs_minecraft_biome::climate::ParameterList<&'static str>,
) -> Vec<ResourceLocation> {
    list.values()
        .iter()
        .filter_map(|(_, biome)| ResourceLocation::parse(biome).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Buf;
    use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};
    use std::path::Path;

    /// The reference's own `possibleBiomes`, per source, from the dump the
    /// feature-order oracle wrote.
    fn dumped_biomes() -> BTreeMap<String, Vec<String>> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../mcrs_minecraft_worldgen_feature/tests/fixtures/vanilla/feature_steps.bin");
        let mut r = open_dump(&path, b"MCFSTEP0");

        let mut sources = BTreeMap::new();
        for _ in 0..r.get_i32_le() {
            let id = dump_string(&mut r);
            let biomes: Vec<String> = (0..r.get_i32_le()).map(|_| dump_string(&mut r)).collect();
            let steps = r.get_i32_le();
            for _ in 0..steps {
                for _ in 0..r.get_i32_le() {
                    dump_string(&mut r);
                }
            }
            for _ in 0..biomes.len() as i32 * steps {
                let skip = r.get_i32_le() as usize;
                r.copy_to_bytes(skip);
            }
            sources.insert(id, biomes);
        }
        assert!(!r.has_remaining(), "trailing bytes in the dump");
        sources
    }

    fn order(source: &BiomeSource) -> Vec<String> {
        possible_biomes(source, |_| None)
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    fn preset(name: &str) -> BiomeSource {
        BiomeSource::MultiNoise(mcrs_minecraft_biome::source::MultiNoiseBiomeSource {
            preset: Some(ResourceLocation::parse(name).unwrap()),
            biomes: None,
        })
    }

    #[test]
    fn every_source_answers_the_biomes_the_reference_does_in_its_order() {
        let dumped = dumped_biomes();
        assert_eq!(
            order(&preset("minecraft:overworld")),
            dumped["minecraft:overworld"]
        );
        assert_eq!(
            order(&preset("minecraft:nether")),
            dumped["minecraft:the_nether"]
        );
        assert_eq!(order(&BiomeSource::TheEnd), dumped["minecraft:the_end"]);
    }
}
