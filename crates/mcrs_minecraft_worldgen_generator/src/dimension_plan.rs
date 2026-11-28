use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_biome::climate::ParameterPoint;
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_game_registries::worldgen::chunk_generator::{
    ChunkGenerator, NoiseChunkGenerator,
};
use mcrs_minecraft_heightmap::HeightmapPredicates;
use mcrs_minecraft_registries::{Diagnostics, IdSet, Namer, Registries, Tags};
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen_feature::compile::build_feature_steps;
use mcrs_minecraft_worldgen_feature::proto::FeatureStepList;
use mcrs_minecraft_worldgen_structure::frozen::DimensionStructureTables;
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;

use crate::SurfaceIds;
use crate::feature_program::FeatureProgram;
use crate::features::{FeatureTables, possible_biomes};
use crate::flat::FlatLayers;
use crate::flat_world::{flat_biome, flat_layers};
use crate::ids::FillIds;
use crate::modern_carvers::{CarverBiomeTable, whole_climate_space};
use crate::multi_noise_biomes::PresetBiomeTables;
use crate::plan_error::PlanError;
use crate::routers::{build_router, refuse_misplaced_beardifier};
use crate::stages::{FillContext, GeneratorSetup, dimension_y_sections, flat_y_sections};
use crate::structures::live_sets;
use crate::world_tables::{FeatureSources, WorldIds, WorldTables};

enum PlannedGenerator {
    Noise {
        router: Arc<NoiseRouter>,
        material: Arc<MaterialProgram>,
    },
    Flat(Arc<FlatLayers>),
}

pub struct DimensionPlan {
    generator: PlannedGenerator,
    blocks: Arc<BlockDefinitions>,
    biome_source: Option<Arc<BiomeSource>>,
    preset_tables: PresetBiomeTables,
    predicates: HeightmapPredicates,
    carvers: Option<Arc<CarverBiomeTable>>,
    surface_ids: Arc<SurfaceIds>,
    fill_ids: FillIds,
    features: Option<Arc<FeatureProgram>>,
    structures: Option<Arc<DimensionStructureTables>>,
}

struct PlanInputs<'a> {
    tables: &'a WorldTables,
    registries: &'a Registries,
    blocks: &'a Blocks,
    seed: u64,
}

struct GeneratorParts {
    generator: PlannedGenerator,
    carvers: Option<Arc<CarverBiomeTable>>,
    structures: Option<Arc<DimensionStructureTables>>,
    features: Option<Arc<FeatureProgram>>,
}

impl DimensionPlan {
    pub fn build(
        tables: &WorldTables,
        registries: &Registries,
        blocks: &Blocks,
        seed: u64,
        dimension: &ResourceLocationBuf,
        generator: &ChunkGenerator,
    ) -> Result<Self, PlanError> {
        let inputs = PlanInputs {
            tables,
            registries,
            blocks,
            seed,
        };
        let blocks = &blocks.0;
        let (parts, biome_source) = match generator {
            ChunkGenerator::Noise(generator) => (
                noise_parts(&inputs, dimension, generator)?,
                Some(Arc::new(generator.biome_source.clone())),
            ),
            ChunkGenerator::Flat(generator) => (
                GeneratorParts {
                    generator: PlannedGenerator::Flat(Arc::new(flat_layers(generator, blocks))),
                    carvers: None,
                    structures: None,
                    features: None,
                },
                flat_biome(generator, registries)
                    .map(|biome| Arc::new(BiomeSource::Fixed { biome })),
            ),
            ChunkGenerator::Debug => return Err(PlanError::NoColumnGenerator),
        };
        let preset_tables = match biome_source.as_deref() {
            Some(BiomeSource::MultiNoise(MultiNoiseBiomeSource::Preset(_))) => {
                tables.preset_tables.as_ref().cloned()
            }
            _ => Ok(PresetBiomeTables::default()),
        };
        match (&tables.ids, preset_tables) {
            (Ok(WorldIds { surface, fill }), Ok(preset_tables)) => Ok(DimensionPlan {
                generator: parts.generator,
                blocks: Arc::clone(blocks),
                biome_source,
                preset_tables,
                predicates: tables.predicates.clone(),
                carvers: parts.carvers,
                surface_ids: Arc::clone(surface),
                fill_ids: fill.clone(),
                features: parts.features,
                structures: parts.structures,
            }),
            (ids, presets) => Err(PlanError::MissingNames(missing_names(
                ids.as_ref().err(),
                presets.err(),
            ))),
        }
    }

    #[must_use]
    pub fn router(&self) -> Option<&NoiseRouter> {
        match &self.generator {
            PlannedGenerator::Noise { router, .. } => Some(router),
            PlannedGenerator::Flat(_) => None,
        }
    }

    #[must_use]
    pub fn fill_context(&self, min_y: i32, section_count: u32) -> FillContext {
        let (setup, y_sections) = match &self.generator {
            PlannedGenerator::Noise { router, material } => (
                GeneratorSetup::Noise {
                    router: Arc::clone(router),
                    material: Some(Arc::clone(material)),
                },
                dimension_y_sections(router, min_y, section_count),
            ),
            PlannedGenerator::Flat(layers) => (
                GeneratorSetup::Flat(Arc::clone(layers)),
                flat_y_sections(min_y, section_count),
            ),
        };
        FillContext::build(
            setup,
            Arc::clone(&self.blocks),
            y_sections,
            self.biome_source.clone(),
            &self.preset_tables,
            self.predicates.clone(),
            self.carvers.clone(),
            Arc::clone(&self.surface_ids),
            &self.fill_ids,
            self.features.clone(),
            self.structures.clone(),
        )
    }
}

fn missing_names(ids: Option<&Diagnostics>, presets: Option<&Diagnostics>) -> Diagnostics {
    let mut diagnostics = Diagnostics::default();
    for missing in [ids, presets].into_iter().flatten() {
        diagnostics.extend(missing.iter().cloned());
    }
    diagnostics
}

fn noise_parts(
    inputs: &PlanInputs<'_>,
    dimension: &ResourceLocationBuf,
    generator: &NoiseChunkGenerator,
) -> Result<GeneratorParts, PlanError> {
    let carvers = carver_table(inputs.tables, dimension, &generator.biome_source)?.map(Arc::new);
    let structures = structure_tables(inputs, dimension, &generator.biome_source);
    let features = match &inputs.tables.features {
        Some(sources) => Some(Arc::new(feature_program(
            inputs,
            sources,
            dimension,
            &generator.biome_source,
            structures.as_deref(),
        )?)),
        None => None,
    };
    let noise_settings = inputs
        .registries
        .registry::<NoiseGeneratorSettings>()
        .ok_or(PlanError::NoNoiseSettingsRegistry)?;
    let (Some(settings), Some(settings_name)) = (
        noise_settings.get(generator.settings),
        noise_settings.name(generator.settings),
    ) else {
        return Err(PlanError::UnknownNoiseSettings {
            settings: generator.settings,
        });
    };
    if let Some(structures) = &structures {
        refuse_misplaced_beardifier(
            inputs.registries,
            dimension,
            settings_name,
            settings,
            structures,
        )?;
    }
    let (router, material) =
        build_router(inputs.registries, settings, inputs.seed, &inputs.blocks.0)
            .map_err(PlanError::RouterCompile)?;
    tracing::info!(%dimension, seed = inputs.seed, "compiled the dimension noise router");
    Ok(GeneratorParts {
        generator: PlannedGenerator::Noise {
            router: Arc::new(router),
            material: Arc::new(material),
        },
        carvers,
        structures,
        features,
    })
}

fn carver_table(
    tables: &WorldTables,
    dimension: &ResourceLocationBuf,
    source: &BiomeSource,
) -> Result<Option<CarverBiomeTable>, PlanError> {
    let Some(carvers) = tables.carvers.as_ref() else {
        return Ok(None);
    };
    let carvers_of = |biome: usize| carvers.get(biome).cloned().unwrap_or_default();
    if let Some(table) = CarverBiomeTable::beta(source, |biome| carvers_of(biome.index())) {
        tracing::info!(%dimension, "resolved the Beta carver table");
        return Ok(Some(table));
    }
    let table = match source {
        BiomeSource::MultiNoise(MultiNoiseBiomeSource::Preset(list)) => {
            let climate = tables
                .preset_tables
                .as_ref()
                .ok()
                .and_then(|presets| presets.get(*list));
            climate.map(|climate| {
                CarverBiomeTable::from_climate(climate.climate(), |biome| {
                    carvers_of(usize::from(biome))
                })
            })
        }
        BiomeSource::MultiNoise(MultiNoiseBiomeSource::Biomes(entries)) => {
            CarverBiomeTable::from_entries(
                entries
                    .iter()
                    .map(|entry| (ParameterPoint::from(&entry.parameters), entry.biome))
                    .collect(),
                |biome| carvers_of(biome.index()),
            )
        }
        BiomeSource::Fixed { biome } => {
            CarverBiomeTable::from_entries(vec![(whole_climate_space(), *biome)], |biome| {
                carvers_of(biome.index())
            })
        }
        _ => return Ok(None),
    };
    match table {
        Some(table) => {
            if table.runs(|carver| matches!(carver, CarverConfig::BetaCave)) {
                return Err(PlanError::BetaCaveOutsideBetaSource {
                    dimension: dimension.clone(),
                });
            }
            tracing::info!(%dimension, entries = table.entry_count(), "resolved the carver table");
            Ok(Some(table))
        }
        None => {
            tracing::info!(%dimension, "no carver table for this biome source");
            Ok(None)
        }
    }
}

fn structure_tables(
    inputs: &PlanInputs<'_>,
    dimension: &ResourceLocationBuf,
    source: &BiomeSource,
) -> Option<Arc<DimensionStructureTables>> {
    let frozen = inputs.tables.structures.as_ref()?;
    let biomes = inputs.registries.registry::<BiomeFile>()?;
    let mut mask = FixedBitSet::with_capacity(biomes.len());
    for name in possible_biomes(source, inputs.registries, &inputs.tables.parameter_lists) {
        if let Some(id) = biomes.by_name(&name) {
            mask.insert(id.index());
        }
    }
    let live = live_sets(frozen, &mask);
    tracing::info!(
        %dimension,
        live_sets = live.len(),
        candidates = live.iter().map(|(_, structures)| structures.len()).sum::<usize>(),
        "resolved the structure sets"
    );
    Some(Arc::new(DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live,
    }))
}

fn decoration_steps(
    steps: &[IdSet<mcrs_minecraft_biome::PlacedFeature>],
    registries: &Registries,
    tags: &Tags<mcrs_minecraft_biome::PlacedFeature>,
) -> Vec<FeatureStepList> {
    steps
        .iter()
        .map(|step| {
            step.ids(tags)
                .filter_map(|id| Some(Holder::Reference(Namer::name(registries, id)?.clone())))
                .collect()
        })
        .collect()
}

fn feature_program(
    inputs: &PlanInputs<'_>,
    sources: &FeatureSources,
    dimension: &ResourceLocationBuf,
    source: &BiomeSource,
    structures: Option<&DimensionStructureTables>,
) -> Result<FeatureProgram, PlanError> {
    let registries = inputs.registries;
    let biome_order = possible_biomes(source, registries, &inputs.tables.parameter_lists);
    let biome_files = registries.registry::<BiomeFile>();
    let steps = biome_order
        .iter()
        .map(|id| {
            biome_files
                .and_then(|biomes| biomes.get(biomes.by_name(id)?))
                .map(|biome| decoration_steps(&biome.features, registries, &sources.placed_tags))
                .ok_or_else(|| PlanError::UnloadedBiome(id.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let entries: Vec<&[FeatureStepList]> = steps.iter().map(Vec::as_slice).collect();
    let built = build_feature_steps(&entries, &sources.loaded).map_err(PlanError::FeatureSteps)?;
    tracing::info!(
        %dimension,
        steps = built.steps.len(),
        features = built.steps.iter().map(Vec::len).sum::<usize>(),
        "resolved the feature steps"
    );
    FeatureProgram::build(
        &FeatureTables {
            features: built,
            biome_order,
            climate: sources.climate.clone(),
        },
        &sources.loaded,
        &inputs.blocks.0,
        registries,
        inputs.seed as i64,
        structures.map(|structure_tables| &*structure_tables.frozen),
    )
    .map_err(PlanError::FeatureProgram)
}
