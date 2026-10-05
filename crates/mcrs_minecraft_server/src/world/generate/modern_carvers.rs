use bevy_ecs::prelude::IntoScheduleConfigs;
use mcrs_minecraft_biome::climate::ParameterPoint;
use mcrs_minecraft_biome::parameter_list::parameter_lists_of;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Entries, EntrySet, Registry, RegistrySet};
use mcrs_minecraft_worldgen::tables::{WorldgenTables, build_worldgen_tables};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_generator::modern_carvers::{
    CarverBiomeTable, resolve_beta_carver_biomes, resolve_carver_biomes, whole_climate_space,
};
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
    biomes: &Registry<keys::Biome>,
    values: &Entries<keys::Biome, mcrs_minecraft_biome::Biome>,
    carvers: &Registry<keys::Carver>,
    table: &Entries<keys::Carver, Option<CarverConfig>>,
) -> Entries<keys::Biome, Arc<[CarverConfig]>> {
    let lists = biomes
        .ids()
        .map(|id| {
            let biome = || biomes.key(id).expect("an id of the registry has a name");
            let set = &values[id].carvers;
            if let EntrySet::Tag(tag) = set {
                tracing::error!(biome = %biome(), %tag, "a carver tag is unsupported until tags have contents");
            }
            set.entries()
                .iter()
                .filter_map(|&carver| match &table[carver] {
                    Some(config) => Some(config.clone()),
                    None => {
                        let carver = carvers.key(carver).expect("an id of the registry has a name");
                        tracing::error!(biome = %biome(), %carver, "a carver of this biome is unavailable");
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
    sources: Option<bevy_ecs::prelude::Res<crate::world::generate::routers::DimensionBiomeSources>>,
    registries: bevy_ecs::prelude::Res<RegistrySet>,
    worldgen: bevy_ecs::prelude::Res<WorldgenTables>,
) {
    let Some(sources) = sources else { return };

    let biomes = registries
        .registry::<keys::Biome>()
        .expect("the data pack loader parses minecraft:worldgen/biome");
    let values = registries
        .entries::<keys::Biome, mcrs_minecraft_biome::Biome>()
        .expect("the data pack loader parses minecraft:worldgen/biome");
    let carver_names = registries
        .registry::<keys::Carver>()
        .expect("the data pack declares minecraft:worldgen/carver");
    let carvers = carvers_by_biome(&biomes, &values, &carver_names, &worldgen.carvers);
    let parameter_lists = parameter_lists_of(&registries);
    let name_of = |id| {
        biomes
            .key(id)
            .expect("an id of the registry has a name")
            .as_str()
            .to_owned()
    };

    let mut tables = DimensionCarverBiomes::default();
    for (dimension, source) in &sources.0 {
        if let BiomeSource::Beta { .. } = source.as_ref() {
            let table = resolve_beta_carver_biomes(source, &biomes, &carvers)
                .expect("a Beta source resolves to a Beta table");
            tracing::info!(%dimension, "resolved the Beta carver table");
            tables.0.insert(dimension.clone(), Arc::new(table));
            continue;
        }

        // A fixed source answers one biome everywhere, so its table is that
        // biome's carvers under a point covering the whole climate space: with a
        // single candidate the nearest-entry search returns it whatever the
        // climate.
        let (preset, fixed_biome) = match source.as_ref() {
            BiomeSource::MultiNoise(multi) => (Some(multi), None),
            BiomeSource::Fixed { biome } => (None, Some(name_of(*biome))),
            _ => continue,
        };

        let explicit = match (preset, fixed_biome) {
            (Some(multi), _) => multi.biomes.as_ref().map(|entries| {
                entries
                    .iter()
                    .map(|entry| {
                        (
                            ParameterPoint::from(&entry.parameters),
                            name_of(entry.biome),
                        )
                    })
                    .collect()
            }),
            (None, Some(biome)) => Some(vec![(whole_climate_space(), biome)]),
            (None, None) => None,
        };

        match resolve_carver_biomes(
            preset.and_then(|multi| multi.preset_in(&parameter_lists)),
            explicit,
            &biomes,
            &carvers,
        ) {
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
