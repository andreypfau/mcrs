use std::collections::BTreeMap;

use bevy_app::AppExit;
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::{Commands, MessageWriter, Res, Resource};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;
use mcrs_minecraft_engine::heightmap::BlockHeightmapPredicates;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_generator::dimension_plan::DimensionPlan;
use mcrs_minecraft_worldgen_generator::plan_error::PlanError;
use mcrs_minecraft_worldgen_generator::world_tables::{MissingTable, WorldTables, WorldgenInputs};
use tracing::{error, warn};

use crate::template_asset::{TemplateAsset, named_templates};
use crate::template_handle::template_handle;
use crate::worldgen_tables::WorldgenTables;

#[derive(Resource, Default)]
pub struct DimensionPlans(pub BTreeMap<ResourceLocationBuf, DimensionPlan>);

pub(crate) fn build_dimension_plans(
    mut commands: Commands,
    (dimensions, list): (Res<DimensionGenerators>, Res<DimensionList>),
    (asset_server, templates): (Res<AssetServer>, Res<Assets<TemplateAsset>>),
    seed: Res<WorldSeed>,
    (blocks, predicates): (Res<Blocks>, Option<Res<BlockHeightmapPredicates>>),
    registries: Res<CurrentRegistries>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(predicates) = predicates else {
        for key in list.keys().iter() {
            error!(dimension = %key.location(), "no heightmap predicates for this dimension; it will generate nothing");
        }
        commands.insert_resource(DimensionPlans::default());
        return;
    };
    let template_names = match named_templates(&registries) {
        Ok(names) => names,
        Err(error) => {
            error!("the loaded registries cannot list their templates: {error}");
            exit.write(AppExit::error());
            return;
        }
    };
    let templates = template_names
        .into_iter()
        .filter_map(|id| {
            let handle = template_handle(&asset_server, &id)?;
            Some((id, templates.get(&handle)?.template.clone()))
        })
        .collect();
    let tables = match WorldTables::build(WorldgenInputs {
        registries: &registries,
        blocks: &blocks,
        templates,
        predicates: predicates.0.clone(),
    }) {
        Ok(tables) => tables,
        Err(error) => {
            error!("{error}");
            exit.write(AppExit::error());
            return;
        }
    };
    for missing in tables.missing() {
        log_missing(missing);
    }

    let mut plans = DimensionPlans::default();
    for key in list.keys().iter() {
        let dimension = key.location();
        let Some(generator) = dimensions.get(&list, key) else {
            warn!(%dimension, "no noise router for this dimension; it will generate nothing");
            continue;
        };
        match DimensionPlan::build(&tables, &registries, &blocks, seed.0, dimension, generator) {
            Ok(plan) => {
                plans.0.insert(dimension.clone(), plan);
            }
            Err(error) if error.is_invalid_data_pack() => {
                error!(%dimension, "{error}");
                exit.write(AppExit::error());
                return;
            }
            Err(error) => log_unplanned(dimension, &error),
        }
    }
    commands.insert_resource(plans);
    commands.insert_resource(WorldgenTables(tables));
}

fn log_missing(missing: &MissingTable) {
    match missing {
        MissingTable::Carvers => error!(
            "the registries declare no carver registry with its tags or no biome registry; no dimension gets a carver table"
        ),
        MissingTable::PresetTables(diagnostics) => error!(
            %diagnostics,
            "no multi-noise preset table; a preset dimension gets no carver table"
        ),
        MissingTable::Structures => error!(
            "the registries declare no structure set, structure, template pool or biome registry; no dimension gets structures"
        ),
        MissingTable::NoiseSettings => error!(
            "the registries declare no noise settings registry; no dimension gets a noise router"
        ),
        MissingTable::Features => error!(
            "the registries declare no feature, placed feature, processor list, block state provider or biome registry; no dimension gets a feature program"
        ),
        other => error!(?other, "a worldgen table is missing"),
    }
}

fn log_unplanned(dimension: &ResourceLocationBuf, error: &PlanError) {
    match error {
        PlanError::MissingNames(diagnostics) => {
            error!(%dimension, %diagnostics, "the registries lack a name worldgen needs; it will generate nothing");
            return;
        }
        PlanError::UnknownNoiseSettings { settings } => error!(
            %dimension, id = ?settings,
            "the dimension names noise settings the registry does not number"
        ),
        PlanError::RouterCompile(error) => error!(
            %dimension, %error,
            "the noise settings did not compile; this dimension will generate nothing"
        ),
        _ => {}
    }
    warn!(%dimension, "no noise router for this dimension; it will generate nothing");
}
