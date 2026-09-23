use mcrs_minecraft_biome::climate::ParameterPoint;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_generator::modern_carvers::CARVER_REGISTRY;
use mcrs_minecraft_worldgen_generator::modern_carvers::{
    CarverBiomeTable, resolve_beta_carver_biomes, resolve_carver_biomes, whole_climate_space,
};
use std::collections::HashMap;
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
            bevy_ecs::prelude::IntoScheduleConfigs::before(
                build_modern_carver_biomes,
                mcrs_minecraft_world::transition_to_playing,
            ),
        );
    }
}

/// Resolve every dimension's biome source climate table into carver lists.
///
/// Both halves are loaded assets: which carvers a biome runs comes from the
/// biome JSON, and what each carver is comes from the carver JSON, so a
/// datapack that retunes either is picked up here.
fn build_modern_carver_biomes(
    mut commands: bevy_ecs::prelude::Commands,
    sources: Option<bevy_ecs::prelude::Res<crate::world::generate::routers::DimensionBiomeSources>>,
    biomes: bevy_ecs::prelude::Res<bevy_asset::Assets<mcrs_minecraft_biome::Biome>>,
    carvers: bevy_ecs::prelude::Res<
        bevy_asset::Assets<mcrs_minecraft_worldgen::bevy::CarverConfigAsset>,
    >,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
) {
    use mcrs_minecraft_assets::snapshot::rl_from_asset_path;

    let Some(sources) = sources else { return };

    let mut config_by_location: HashMap<String, CarverConfig> = HashMap::new();
    for (asset_id, asset) in carvers.iter() {
        let Some(path) = asset_server.get_path(asset_id) else {
            continue;
        };
        let Some(location) = rl_from_asset_path(path.path(), CARVER_REGISTRY) else {
            continue;
        };
        config_by_location.insert(location.as_str().to_owned(), asset.config.clone());
    }

    let mut carvers_by_biome: HashMap<String, Vec<String>> = HashMap::new();
    for (asset_id, biome) in biomes.iter() {
        let Some(path) = asset_server.get_path(asset_id) else {
            continue;
        };
        let Some(location) = rl_from_asset_path(path.path(), "worldgen/biome") else {
            continue;
        };
        carvers_by_biome.insert(
            location.as_str().to_owned(),
            biome
                .carvers
                .iter()
                .map(|carver| carver.as_str().to_owned())
                .collect(),
        );
    }

    let mut tables = DimensionCarverBiomes::default();
    for (dimension, source) in &sources.0 {
        if let BiomeSource::Beta { .. } = source.as_ref() {
            let table = resolve_beta_carver_biomes(source, &carvers_by_biome, &config_by_location)
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
            BiomeSource::Fixed { biome_id, .. } => (None, Some(biome_id.as_str().to_owned())),
            _ => continue,
        };

        let explicit = match (preset, fixed_biome) {
            (Some(multi), _) => multi.biomes.as_ref().map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| {
                        let path = asset_server.get_path(entry.biome.id())?;
                        let location = rl_from_asset_path(path.path(), "worldgen/biome")?;
                        Some((
                            ParameterPoint::from(&entry.parameters),
                            location.as_str().to_owned(),
                        ))
                    })
                    .collect()
            }),
            (None, Some(biome)) => Some(vec![(whole_climate_space(), biome)]),
            (None, None) => None,
        };

        match resolve_carver_biomes(
            preset
                .and_then(|multi| multi.preset.as_ref())
                .map(|preset| preset.as_str()),
            explicit,
            &carvers_by_biome,
            &config_by_location,
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
