use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

use mcrs_minecraft_biome::TemperatureModifier;
use mcrs_minecraft_biome::parameter_list::{ParameterLists, parameter_lists_of};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider;
use mcrs_minecraft_entity::variant::{CatSoundVariant, ChickenSoundVariant};
use mcrs_minecraft_game_registries::variant::{
    CatVariantFile, ChickenVariantFile, ZombieNautilusVariantFile, spawn_selectors,
};
use mcrs_minecraft_heightmap::HeightmapPredicates;
use mcrs_minecraft_registries::{Diagnostics, Named, Namer, Registries, Registry, Resolve, Tags};
use mcrs_minecraft_resource::{Registered, RegistryValue, ResourceLocationBuf};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::compile::LoadedFeatures;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_feature::template::{PaletteState, Template};
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;
use mcrs_minecraft_worldgen_structure::frozen::FrozenStructures;
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};

use crate::SurfaceIds;
use crate::ids::FillIds;
use crate::multi_noise_biomes::PresetBiomeTables;
use crate::structures::{StructureInputs, VariantInputs, freeze, resolve_palette_state};
use crate::world_tables_error::WorldTablesError;

pub struct WorldgenInputs<'a> {
    pub registries: &'a Registries,
    pub blocks: &'a Blocks,
    pub templates: BTreeMap<ResourceLocationBuf, Template>,
    pub predicates: HeightmapPredicates,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MissingTable {
    Carvers,
    PresetTables(Diagnostics),
    Structures,
    NoiseSettings,
    Features,
}

pub struct WorldTables {
    pub(crate) parameter_lists: ParameterLists,
    pub(crate) carvers: Option<Vec<Arc<[CarverConfig]>>>,
    pub(crate) preset_tables: Result<PresetBiomeTables, Diagnostics>,
    pub(crate) ids: Result<WorldIds, Diagnostics>,
    pub(crate) structures: Option<Arc<FrozenStructures>>,
    pub(crate) features: Option<FeatureSources>,
    pub(crate) predicates: HeightmapPredicates,
    missing: Vec<MissingTable>,
}

pub(crate) struct WorldIds {
    pub(crate) surface: Arc<SurfaceIds>,
    pub(crate) fill: FillIds,
}

pub(crate) struct FeatureSources {
    pub(crate) loaded: LoadedFeatures,
    pub(crate) placed_tags: Tags<mcrs_minecraft_biome::PlacedFeature>,
    pub(crate) climate: BTreeMap<ResourceLocationBuf, BiomeClimate>,
}

impl WorldTables {
    pub fn build(inputs: WorldgenInputs<'_>) -> Result<Self, WorldTablesError> {
        let WorldgenInputs {
            registries,
            blocks,
            templates,
            predicates,
        } = inputs;
        let mut missing = Vec::new();
        let carvers = carvers_by_biome(registries);
        let preset_tables = preset_tables(registries);
        match (&carvers, &preset_tables) {
            (None, _) => missing.push(MissingTable::Carvers),
            (Some(_), Err(diagnostics)) => {
                missing.push(MissingTable::PresetTables(diagnostics.clone()));
            }
            (Some(_), Ok(_)) => {}
        }
        let structures = freeze_structures(registries, blocks, &templates)?.map(Arc::new);
        if structures.is_none() {
            missing.push(MissingTable::Structures);
        }
        if registries.registry::<NoiseGeneratorSettings>().is_none() {
            missing.push(MissingTable::NoiseSettings);
        }
        let features = feature_sources(registries, templates);
        if features.is_none() {
            missing.push(MissingTable::Features);
        }
        Ok(WorldTables {
            parameter_lists: parameter_lists_of(registries)
                .unwrap_or_else(|| Arc::from(Vec::new())),
            carvers,
            preset_tables,
            ids: world_ids(registries),
            structures,
            features,
            predicates,
            missing,
        })
    }

    #[must_use]
    pub fn missing(&self) -> &[MissingTable] {
        &self.missing
    }
}

fn preset_tables(registries: &Registries) -> Result<PresetBiomeTables, Diagnostics> {
    let mut diagnostics = Diagnostics::default();
    PresetBiomeTables::resolve(registries, &mut diagnostics).ok_or(diagnostics)
}

fn world_ids(registries: &Registries) -> Result<WorldIds, Diagnostics> {
    let mut diagnostics = Diagnostics::default();
    let surface = SurfaceIds::resolve(registries, &mut diagnostics);
    let fill = FillIds::resolve(registries, &mut diagnostics);
    match (surface, fill) {
        (Some(surface), Some(fill)) => Ok(WorldIds {
            surface: Arc::new(surface),
            fill,
        }),
        _ => Err(diagnostics),
    }
}

fn carvers_by_biome(registries: &Registries) -> Option<Vec<Arc<[CarverConfig]>>> {
    let carvers = registries.registry::<CarverConfig>()?;
    let carver_tags = registries.tags::<mcrs_minecraft_biome::ConfiguredCarver>()?;
    let biomes = registries.registry::<BiomeFile>()?;
    Some(
        biomes
            .values()
            .iter()
            .map(|biome| {
                biome
                    .carvers
                    .ids(&carver_tags)
                    .filter_map(|carver| carvers.get(carver).cloned())
                    .collect()
            })
            .collect(),
    )
}

fn named_entries<V: RegistryValue + Clone>(
    registry: &Registry<V>,
) -> BTreeMap<ResourceLocationBuf, V> {
    registry
        .iter()
        .map(|(_, name, value)| (name.clone(), value.clone()))
        .collect()
}

fn names<R: Registered>(registries: &Registries) -> Vec<ResourceLocationBuf> {
    registries
        .entries::<R>()
        .map(|entries| entries.names().to_vec())
        .unwrap_or_default()
}

fn freeze_structures(
    registries: &Registries,
    blocks: &Blocks,
    templates: &BTreeMap<ResourceLocationBuf, Template>,
) -> Result<Option<FrozenStructures>, WorldTablesError> {
    let (Some(sets), Some(structures), Some(pools), Some(_)) = (
        registries.registry::<StructureSet>(),
        registries.registry::<Structure>(),
        registries.registry::<TemplatePool>(),
        registries.registry::<BiomeFile>(),
    ) else {
        return Ok(None);
    };
    let sets = named_entries(sets);
    let structures = named_entries(structures);
    let pools = named_entries(pools);
    let template = |id: &ResourceLocationBuf| templates.get(id).map(Cow::Borrowed);
    let resolve = |state: &PaletteState| resolve_palette_state(&blocks.0, state);
    let cats =
        spawn_selectors::<CatVariantFile>(registries).map_err(WorldTablesError::SpawnConditions)?;
    let chickens = spawn_selectors::<ChickenVariantFile>(registries)
        .map_err(WorldTablesError::SpawnConditions)?;
    let zombie_nautiluses = spawn_selectors::<ZombieNautilusVariantFile>(registries)
        .map_err(WorldTablesError::SpawnConditions)?;
    let cat_sounds = names::<CatSoundVariant>(registries);
    let chicken_sounds = names::<ChickenSoundVariant>(registries);
    let frozen = freeze(&StructureInputs {
        sets: &sets,
        structures: &structures,
        pools: &pools,
        template: &template,
        resolve: &resolve,
        registries,
        variants: &VariantInputs {
            cats: Some(&cats),
            cat_sounds: &cat_sounds,
            chickens: Some(&chickens),
            chicken_sounds: &chicken_sounds,
            zombie_nautiluses: Some(&zombie_nautiluses),
        },
    })
    .map_err(WorldTablesError::StructureFreeze)?;
    tracing::info!(
        sets = frozen.sets.len(),
        structures = frozen.structures.len(),
        pools = frozen.pools.len(),
        templates = frozen.templates.len(),
        "froze the structure registries"
    );
    Ok(Some(frozen))
}

fn placed_feature_tags(
    registries: &Registries,
    tags: &Tags<mcrs_minecraft_biome::PlacedFeature>,
) -> BTreeMap<ResourceLocationBuf, Vec<ResourceLocationBuf>> {
    tags.iter()
        .map(|(tag, name)| {
            let members = tags
                .members(tag)
                .filter_map(|id| Namer::name(registries, id).cloned())
                .collect();
            (name.clone(), members)
        })
        .collect()
}

fn named_providers(
    registries: &Registries,
    providers: &Registry<DirectBlockStateProvider>,
) -> BTreeMap<ResourceLocationBuf, DirectBlockStateProvider<Named>> {
    providers
        .iter()
        .filter_map(|(_, name, provider)| match provider.name(registries) {
            Ok(provider) => Some((name.clone(), provider)),
            Err(error) => {
                tracing::error!(%name, %error, "a block state provider cannot be named");
                None
            }
        })
        .collect()
}

fn loaded_features(
    registries: &Registries,
    templates: BTreeMap<ResourceLocationBuf, Template>,
) -> Option<LoadedFeatures> {
    Some(LoadedFeatures {
        features: named_entries(registries.registry::<Feature>()?),
        placed_features: named_entries(registries.registry::<PlacedFeature>()?),
        templates,
        processor_lists: named_entries(registries.registry::<StructureProcessorList>()?),
        block_state_providers: named_providers(registries, registries.registry()?),
        placed_feature_tags: placed_feature_tags(registries, &registries.tags()?),
    })
}

fn feature_sources(
    registries: &Registries,
    templates: BTreeMap<ResourceLocationBuf, Template>,
) -> Option<FeatureSources> {
    let loaded = loaded_features(registries, templates)?;
    let placed_tags = registries.tags::<mcrs_minecraft_biome::PlacedFeature>()?;
    let climate = registries
        .registry::<BiomeFile>()?
        .iter()
        .map(|(_, name, biome)| {
            (
                name.clone(),
                BiomeClimate {
                    base_temperature: biome.temperature,
                    frozen: biome.temperature_modifier == Some(TemperatureModifier::Frozen),
                    has_precipitation: biome.has_precipitation,
                },
            )
        })
        .collect();
    Some(FeatureSources {
        loaded,
        placed_tags,
        climate,
    })
}
