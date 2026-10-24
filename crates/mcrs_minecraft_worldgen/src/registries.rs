use std::collections::BTreeMap;

use mcrs_minecraft_biome_file::{BiomeFile, NetworkBiome};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{PackFiles, VANILLA_PACK};
use mcrs_minecraft_noise::proto::NoiseParam;
use mcrs_minecraft_registries::{
    DataDeclaration, DataValue, Declaration, RegistryAdditions, insert_entry,
};
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::biome_domain::check_feature_domains;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};
use mcrs_minecraft_worldgen_surface::proto::{MaterialConditionHolder, MaterialRuleHolder};

const STATIC_REGISTRIES: [&[(
    mcrs_minecraft_core::StaticResourceLocation,
    &[mcrs_minecraft_core::StaticResourceLocation],
)]; 5] = [
    mcrs_minecraft_worldgen_carver::keys::STATIC_REGISTRIES,
    mcrs_minecraft_worldgen_density::keys::STATIC_REGISTRIES,
    mcrs_minecraft_worldgen_feature::keys::STATIC_REGISTRIES,
    mcrs_minecraft_worldgen_structure::keys::STATIC_REGISTRIES,
    mcrs_minecraft_worldgen_surface::keys::STATIC_REGISTRIES,
];

/// Adds the registries of vanilla world generation to `additions`: the static registries of the
/// worldgen crates, the data registries the generator reads, and the entries the game and the
/// beta preset build by code. The biome declaration replaces the vanilla one, so a biome also
/// checks the domain of the features it places.
pub fn add_vanilla_worldgen(additions: &mut RegistryAdditions) {
    for (key, names) in STATIC_REGISTRIES.into_iter().flatten() {
        let names = names.iter().map(|&name| name.into()).collect();
        additions
            .declarations
            .push(Declaration::fixed((*key).into(), names));
    }
    additions.declarations.extend([
        biome_declaration().synced_via::<NetworkBiome>().into(),
        Declaration::data::<Feature>().into(),
        Declaration::data::<PlacedFeature>().into(),
        Declaration::data::<StructureProcessorList>().into(),
        Declaration::data::<Structure>().into(),
        Declaration::data::<StructureSet>().into(),
        Declaration::data::<CarverConfig>().into(),
        Declaration::data::<TemplatePool>().into(),
        Declaration::data::<NoiseParam>().into(),
        Declaration::data::<DensityFunctionHolder>().into(),
        Declaration::data::<NoiseGeneratorSettings>().into(),
        Declaration::data::<MaterialRuleHolder>().into(),
        Declaration::data::<MaterialConditionHolder>().into(),
    ]);
    insert_built_ins(&mut additions.files);
}

fn biome_declaration() -> DataDeclaration<BiomeFile> {
    Declaration::data::<BiomeFile>().validate(|registry, registries| {
        let (Some(placed), Some(tags)) = (
            registries.registry::<PlacedFeature>(),
            registries.tags::<mcrs_minecraft_biome::PlacedFeature>(),
        ) else {
            return Vec::new();
        };
        let biomes = registry.values().iter().map(|biome| {
            biome
                .features
                .iter()
                .map(|step| {
                    step.ids(&tags)
                        .filter_map(|id| Some((placed.name(id)?, placed.get(id)?)))
                        .collect()
                })
                .collect()
        });
        check_feature_domains(biomes)
    })
}

const BETA_PACK: &str = "file/beta";

fn insert_built_ins(files: &mut PackFiles) {
    insert_all::<BiomeFile>(files, VANILLA_PACK, builtin::biomes());
    insert_all::<TemplatePool>(files, VANILLA_PACK, builtin::template_pools());
    insert_all::<NoiseParam>(files, VANILLA_PACK, builtin::vanilla_noises());
    insert_all::<NoiseParam>(files, BETA_PACK, builtin::beta_noises());
    insert_all::<DensityFunctionHolder>(files, VANILLA_PACK, builtin::vanilla_density_functions());
    insert_all::<DensityFunctionHolder>(files, BETA_PACK, builtin::beta_density_functions());
    insert_all::<NoiseGeneratorSettings>(files, VANILLA_PACK, builtin::vanilla_noise_settings());
    insert_all::<NoiseGeneratorSettings>(files, BETA_PACK, builtin::beta_noise_settings());
}

fn insert_all<V: DataValue>(
    files: &mut PackFiles,
    pack: &str,
    values: BTreeMap<ResourceLocation, V::Named>,
) where
    V::Named: Send + Sync,
{
    for (name, value) in values {
        insert_entry::<V>(files, pack, &name, value);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_world::registry_load::test_registries;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct RegistryReport {
        entries: BTreeMap<String, EntryReport>,
    }

    #[derive(Deserialize)]
    struct EntryReport {
        protocol_id: u16,
    }

    #[test]
    fn every_reported_static_registry_is_declared_with_its_entries_in_protocol_id_order() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        );
        let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let report: BTreeMap<String, RegistryReport> = serde_json::from_str(&text).unwrap();
        for (name, registry) in report {
            let mut reported: Vec<(u16, String)> = (registry.entries.into_iter())
                .map(|(entry, report)| (report.protocol_id, entry))
                .collect();
            reported.sort();
            let declared = test_registries()
                .names_of(&ResourceLocation::read(&name).unwrap())
                .unwrap_or_else(|| panic!("{name} is not declared"));
            let declared: Vec<String> = declared.iter().map(ToString::to_string).collect();
            let reported: Vec<String> = reported.into_iter().map(|(_, entry)| entry).collect();
            assert_eq!(declared, reported, "{name}");
        }
    }
}
