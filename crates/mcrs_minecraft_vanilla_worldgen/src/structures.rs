use crate::template_asset::{TemplateAsset, named_templates};
use crate::template_handle::template_handle;
use bevy_app::{App, AppExit, Plugin};
use bevy_asset::{AssetServer, Assets, Handle};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, MessageWriter, Res, Resource};
use bevy_state::prelude::OnEnter;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_biome::parameter_list::{ParameterLists, parameter_lists_of};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_engine_world::world::sub_app::DimSpawnEnqueue;
use mcrs_minecraft_entity::variant::{CatSoundVariant, ChickenSoundVariant};
use mcrs_minecraft_game_registries::variant::spawn_selectors;
use mcrs_minecraft_game_registries::variant::{
    CatVariantFile, ChickenVariantFile, ZombieNautilusVariantFile,
};
use mcrs_minecraft_game_registries::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_registries::{CurrentRegistries, Registries, Registry};
use mcrs_minecraft_resource::{Registered, ResourceLocationBuf};
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::template::PaletteState;
use mcrs_minecraft_worldgen_generator::features::possible_biomes;
use mcrs_minecraft_worldgen_generator::structures::{
    StructureInputs, VariantInputs, freeze, live_sets, resolve_palette_state,
};
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, FrozenStructures};
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Resource, Default, Clone)]
pub struct DimensionStructures(pub BTreeMap<ResourceLocationBuf, Arc<DimensionStructureTables>>);

pub fn dimension_tables(
    frozen: Arc<FrozenStructures>,
    biomes: &Registry<BiomeFile>,
    registries: &Registries,
    lists: &ParameterLists,
    dimensions: &DimensionGenerators,
    list: &DimensionList,
) -> DimensionStructures {
    let mut tables = DimensionStructures::default();
    for (dimension, generator) in dimensions.iter(list) {
        let ChunkGenerator::Noise(generator) = generator else {
            continue;
        };
        let dimension = dimension.location();
        let mut mask = FixedBitSet::with_capacity(biomes.len());
        for name in possible_biomes(&generator.biome_source, registries, lists) {
            if let Some(id) = biomes.by_name(&name) {
                mask.insert(id.index());
            }
        }
        let live = live_sets(&frozen, &mask);
        tracing::info!(
            %dimension,
            live_sets = live.len(),
            candidates = live.iter().map(|(_, structures)| structures.len()).sum::<usize>(),
            "resolved the structure sets"
        );
        tables.0.insert(
            dimension.clone(),
            Arc::new(DimensionStructureTables {
                frozen: Arc::clone(&frozen),
                live,
            }),
        );
    }
    tables
}

pub struct StructurePlugin;

impl Plugin for StructurePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Playing),
            build_dimension_structures.before(DimSpawnEnqueue),
        );
    }
}

pub(crate) fn build_dimension_structures(
    mut commands: Commands,
    (dimensions, list): (Res<DimensionGenerators>, Res<DimensionList>),
    asset_server: Res<AssetServer>,
    templates: Res<Assets<TemplateAsset>>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(sets), Some(structures), Some(pools), Some(biomes)) = (
        registries.registry::<StructureSet>(),
        registries.registry::<Structure>(),
        registries.registry::<TemplatePool>(),
        registries.registry::<BiomeFile>(),
    ) else {
        tracing::error!(
            "the registries declare no structure set, structure, template pool or biome registry; no dimension gets structures"
        );
        return;
    };
    let sets = sets
        .iter()
        .map(|(_, name, set)| (name.clone(), set.clone()))
        .collect();
    let structures = structures
        .iter()
        .map(|(_, name, structure)| (name.clone(), structure.clone()))
        .collect();
    let pools = pools
        .iter()
        .map(|(_, name, pool)| (name.clone(), pool.clone()))
        .collect();
    let template_names = match named_templates(&registries) {
        Ok(names) => names,
        Err(error) => {
            tracing::error!("the loaded registries cannot list their templates: {error}");
            exit.write(AppExit::error());
            return;
        }
    };
    let template_handles: BTreeMap<ResourceLocationBuf, Handle<TemplateAsset>> = template_names
        .into_iter()
        .filter_map(|id| Some((id.clone(), template_handle(&asset_server, &id)?)))
        .collect();

    let template = |id: &ResourceLocationBuf| {
        let handle = template_handles.get(id)?;
        Some(Cow::Borrowed(&templates.get(handle)?.template))
    };
    let resolve = |state: &PaletteState| resolve_palette_state(&blocks.0, state);
    let selectors = (|| -> Result<_, String> {
        Ok((
            spawn_selectors::<CatVariantFile>(&registries)?,
            spawn_selectors::<ChickenVariantFile>(&registries)?,
            spawn_selectors::<ZombieNautilusVariantFile>(&registries)?,
        ))
    })();
    let (cats, chickens, zombie_nautiluses) = match selectors {
        Ok(selectors) => selectors,
        Err(error) => {
            tracing::error!("the loaded registries cannot list their spawn conditions: {error}");
            exit.write(AppExit::error());
            return;
        }
    };
    fn names<R: Registered>(registries: &Registries) -> Vec<ResourceLocationBuf> {
        registries
            .entries::<R>()
            .map(|entries| entries.names().to_vec())
            .unwrap_or_default()
    }
    let cat_sounds = names::<CatSoundVariant>(&registries);
    let chicken_sounds = names::<ChickenSoundVariant>(&registries);
    let frozen = match freeze(&StructureInputs {
        sets: &sets,
        structures: &structures,
        pools: &pools,
        template: &template,
        resolve: &resolve,
        registries: &registries,
        variants: &VariantInputs {
            cats: Some(&cats),
            cat_sounds: &cat_sounds,
            chickens: Some(&chickens),
            chicken_sounds: &chicken_sounds,
            zombie_nautiluses: Some(&zombie_nautiluses),
        },
    }) {
        Ok(frozen) => frozen,
        Err(error) => {
            tracing::error!("the structure registries do not resolve: {error}");
            exit.write(AppExit::error());
            return;
        }
    };
    tracing::info!(
        sets = frozen.sets.len(),
        structures = frozen.structures.len(),
        pools = frozen.pools.len(),
        templates = frozen.templates.len(),
        "froze the structure registries"
    );
    commands.insert_resource(dimension_tables(
        Arc::new(frozen),
        biomes,
        &registries,
        &parameter_lists_of(&registries).unwrap_or_else(|| Arc::from(Vec::new())),
        &dimensions,
        &list,
    ));
}
