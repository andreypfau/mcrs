use bevy_ecs::prelude::IntoScheduleConfigs;
use mcrs_minecraft_biome::climate::ParameterPoint;
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_game_registries::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_registries::{CurrentRegistries, Diagnostics, Registries};
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_generator::modern_carvers::{CarverBiomeTable, whole_climate_space};
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::PresetBiomeTables;
use std::sync::Arc;

#[derive(bevy_ecs::prelude::Resource, Default, Clone)]
pub struct DimensionCarverBiomes(
    pub std::collections::BTreeMap<ResourceLocationBuf, Arc<CarverBiomeTable>>,
);

pub struct ModernCarverPlugin;

impl bevy_app::Plugin for ModernCarverPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            bevy_state::prelude::OnEnter(mcrs_minecraft_assets::AppState::WorldgenFreeze),
            build_modern_carver_biomes
                .before(mcrs_minecraft_engine_registries::transition_to_playing),
        );
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

fn build_modern_carver_biomes(
    mut commands: bevy_ecs::prelude::Commands,
    dimensions: bevy_ecs::prelude::Res<
        mcrs_minecraft_engine::dimension_generators::DimensionGenerators,
    >,
    list: bevy_ecs::prelude::Res<mcrs_minecraft_engine_world::world::dimension_list::DimensionList>,
    current: bevy_ecs::prelude::Res<CurrentRegistries>,
) {
    let Some(carvers) = carvers_by_biome(&current) else {
        tracing::error!(
            "the registries declare no carver registry with its tags or no biome registry; no dimension gets a carver table"
        );
        return;
    };
    let mut diagnostics = Diagnostics::default();
    let preset_tables = PresetBiomeTables::resolve(&current, &mut diagnostics);
    if preset_tables.is_none() {
        tracing::error!(%diagnostics, "no multi-noise preset table; a preset dimension gets no carver table");
    }
    let carvers_of = |biome: usize| carvers.get(biome).cloned().unwrap_or_default();

    let mut tables = DimensionCarverBiomes::default();
    for (dimension, generator) in dimensions.iter(&list) {
        let ChunkGenerator::Noise(generator) = generator else {
            continue;
        };
        let dimension = dimension.location();
        let source = &generator.biome_source;
        if let Some(table) = CarverBiomeTable::beta(source, |biome| carvers_of(biome.index())) {
            tracing::info!(%dimension, "resolved the Beta carver table");
            tables.0.insert(dimension.clone(), Arc::new(table));
            continue;
        }

        let table = match source {
            BiomeSource::MultiNoise(MultiNoiseBiomeSource::Preset(list)) => {
                let Some(climate) = preset_tables.as_ref().and_then(|tables| tables.get(*list))
                else {
                    continue;
                };
                Some(CarverBiomeTable::from_climate(climate.climate(), |biome| {
                    carvers_of(usize::from(biome))
                }))
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
            _ => continue,
        };

        match table {
            Some(table) => {
                assert!(
                    !table.runs(|carver| matches!(carver, CarverConfig::BetaCave)),
                    "{dimension}: mcrs:beta_cave carves only over a Beta biome source"
                );
                tracing::info!(%dimension, entries = table.entry_count(), "resolved the carver table");
                tables.0.insert(dimension.clone(), Arc::new(table));
            }
            None => tracing::info!(%dimension, "no carver table for this biome source"),
        }
    }
    commands.insert_resource(tables);
}
