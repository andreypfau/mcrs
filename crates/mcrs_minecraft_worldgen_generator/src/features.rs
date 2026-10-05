use mcrs_minecraft_biome::parameter_list::ParameterLists;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::Registry;
use mcrs_minecraft_worldgen_feature::compile::FeatureSteps;
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;
use std::collections::BTreeMap;

/// `TheEndBiomeSource` lists its five biomes in this order, and that order is
/// the input of the sort.
const END_BIOMES: [ResourceKey<keys::Biome, &'static str>; 5] = [
    keys::biome::THE_END,
    keys::biome::END_HIGHLANDS,
    keys::biome::END_MIDLANDS,
    keys::biome::SMALL_END_ISLANDS,
    keys::biome::END_BARRENS,
];

/// One dimension's sorted feature tables: what [`FeatureProgram::build`]
/// resolves into numbers.
#[derive(Clone)]
pub struct FeatureTables {
    pub features: FeatureSteps,
    /// The biome source's own order, which decides the sort.
    pub biome_order: Vec<ResourceLocation>,
    /// What each biome's climate says about freezing. Only the parsed biome
    /// carries it, and the registry the feature program builds against holds
    /// names alone, so it is read here and carried rather than resolved there.
    pub climate: BTreeMap<ResourceLocation, BiomeClimate>,
}

/// The biomes a source can answer with, in the source's own order and without
/// repeats — the input the feature order is defined against.
pub fn possible_biomes(
    source: &BiomeSource,
    biomes: &Registry<keys::Biome>,
    lists: &ParameterLists,
) -> Vec<ResourceLocation> {
    let named = |id: &mcrs_minecraft_registry::Id<keys::Biome>| {
        biomes
            .name(*id)
            .unwrap_or_else(|| panic!("the biome registry holds no entry numbered {}", id.index()))
            .clone()
    };
    let listed: Vec<ResourceLocation> = match source {
        BiomeSource::MultiNoise(multi) => match (&multi.biomes, &multi.preset) {
            (Some(entries), _) => entries.iter().map(|entry| named(&entry.biome)).collect(),
            (None, Some(list)) => match lists.get(*list) {
                Some(list) => preset_biomes(list.preset.parameter_list()),
                None => panic!("no parameter list is numbered {}", list.index()),
            },
            (None, None) => panic!("a multi-noise source names neither biomes nor a preset"),
        },
        BiomeSource::TheEnd => END_BIOMES
            .iter()
            .map(|biome| biome.location().to_arc())
            .collect(),
        BiomeSource::Fixed { biome } => vec![named(biome)],
        BiomeSource::Checkerboard { biomes, .. } => biomes.iter().map(named).collect(),
        BiomeSource::Beta { land_biomes, .. } => land_biomes.iter().map(named).collect(),
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
        .filter_map(|(_, biome)| ResourceLocation::read(biome).ok())
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
        let biomes = Registry::<keys::Biome>::new([]).unwrap();
        possible_biomes(source, &biomes, &crate::tests::parameter_lists().1)
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    fn preset(name: &str) -> BiomeSource {
        BiomeSource::MultiNoise(mcrs_minecraft_biome::source::MultiNoiseBiomeSource {
            preset: Some(crate::tests::parameter_list_id(name)),
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
