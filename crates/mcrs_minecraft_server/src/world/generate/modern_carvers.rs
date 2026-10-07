use crate::loaded::Loaded;
use bevy_ecs::prelude::IntoScheduleConfigs;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::climate::ParameterPoint;
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_registry::shared::Resolved;
use mcrs_minecraft_registry::{Entries, Registry, RegistrySet, Tags};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_worldgen::tables::{WorldgenTables, build_worldgen_tables};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_generator::modern_carvers::{CarverBiomeTable, whole_climate_space};
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::PresetBiomeTables;
use std::sync::Arc;

/// Every dimension's carver table, keyed the way its biome source is: a table
/// resolves one source's climate entries, so the Nether's carvers are not the
/// overworld's.
#[derive(bevy_ecs::prelude::Resource, Default, Clone)]
pub struct DimensionCarverBiomes(
    pub std::collections::BTreeMap<mcrs_minecraft_core::ResourceLocation, Arc<CarverBiomeTable>>,
);

pub struct ModernCarverPlugin;

impl bevy_app::Plugin for ModernCarverPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            bevy_state::prelude::OnEnter(mcrs_minecraft_assets::AppState::WorldgenFreeze),
            build_modern_carver_biomes
                .before(mcrs_minecraft_world::transition_to_playing)
                .after(build_worldgen_tables),
        );
    }
}

/// The carvers every biome runs, in the order its file lists them. A carver a
/// biome names that did not load is reported and the biome runs without it.
fn carvers_by_biome(
    biomes: &Registry<Biome>,
    values: &Entries<Biome, mcrs_minecraft_biome_file::BiomeGenerationSettings>,
    carvers: &Registry<CarverConfig>,
    carver_tags: &Tags<CarverConfig>,
    table: &Entries<CarverConfig, Option<CarverConfig>>,
) -> Entries<Biome, Arc<[CarverConfig]>> {
    let lists = biomes
        .iter()
        .map(|(id, biome)| {
            values[id]
                .carvers
                .ids(carver_tags)
                .filter_map(|carver| match &table[carver] {
                    Some(config) => Some(config.clone()),
                    None => {
                        let carver = carvers
                            .name(carver)
                            .expect("an id of the registry has a name");
                        tracing::error!(%biome, %carver, "a carver of this biome is unavailable");
                        None
                    }
                })
                .collect()
        })
        .collect();
    Entries::new(biomes, lists).expect("one list for every biome")
}

/// Resolve every dimension's biome source climate table into carver lists.
///
/// Both halves are loaded values: which carvers a biome runs comes from the
/// biome JSON, and what each carver is comes from the carver table, so a
/// datapack that retunes either is picked up here.
fn build_modern_carver_biomes(
    mut commands: bevy_ecs::prelude::Commands,
    dimensions: bevy_ecs::prelude::Res<crate::world_options::DimensionList>,
    registries: bevy_ecs::prelude::Res<RegistrySet>,
    worldgen: bevy_ecs::prelude::Res<WorldgenTables>,
    preset_tables: bevy_ecs::prelude::Res<Resolved<PresetBiomeTables>>,
) {
    let biomes = registries.loaded_registry::<Biome>();
    let values =
        registries.loaded_entries::<Biome, mcrs_minecraft_biome_file::BiomeGenerationSettings>();
    let carver_names = registries.loaded_registry::<CarverConfig>();
    let carver_tags = registries.loaded_tags::<CarverConfig>();
    let carvers = carvers_by_biome(
        &biomes,
        &values,
        &carver_names,
        &carver_tags,
        &worldgen.carvers,
    );

    let mut tables = DimensionCarverBiomes::default();
    for (dimension, entry) in dimensions.iter() {
        let ChunkGenerator::Noise(generator) = &entry.generator else {
            continue;
        };
        let dimension = dimension.location();
        let source = &generator.biome_source;
        if let BiomeSource::Beta { .. } = source {
            let table = CarverBiomeTable::beta(source, |biome| carvers[biome].clone())
                .expect("a Beta source resolves to a Beta table");
            tracing::info!(%dimension, "resolved the Beta carver table");
            tables.0.insert(dimension.clone(), Arc::new(table));
            continue;
        }

        // A fixed source answers one biome everywhere, so its table is that
        // biome's carvers under a point covering the whole climate space: with a
        // single candidate the nearest-entry search returns it whatever the
        // climate.
        let table = match source {
            BiomeSource::MultiNoise(MultiNoiseBiomeSource::Preset(list)) => {
                let climate = preset_tables
                    .get(*list)
                    .expect("the resolved tables hold every parameter list");
                Some(CarverBiomeTable::from_climate(climate.climate(), |biome| {
                    carvers.as_slice()[usize::from(biome)].clone()
                }))
            }
            BiomeSource::MultiNoise(MultiNoiseBiomeSource::Biomes(entries)) => {
                CarverBiomeTable::from_entries(
                    entries
                        .iter()
                        .map(|entry| (ParameterPoint::from(&entry.parameters), entry.biome))
                        .collect(),
                    |biome| carvers[*biome].clone(),
                )
            }
            BiomeSource::Fixed { biome } => {
                CarverBiomeTable::from_entries(vec![(whole_climate_space(), *biome)], |biome| {
                    carvers[*biome].clone()
                })
            }
            _ => continue,
        };

        match table {
            Some(table) => {
                // Beta's caves abort on water and leave Beta's substance, which
                // only the Beta column program answers.
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
