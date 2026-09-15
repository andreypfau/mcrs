use crate::world::generate::features::registry_of;
use crate::world::generate::routers::DimensionBiomeSources;
use bevy_app::{App, Plugin};
use bevy_asset::{AssetServer, Assets, Handle};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_state::prelude::OnEnter;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::snapshot::rl_from_asset_path;
use mcrs_minecraft_assets::{AppState, DynTagRegistry};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::DynRegistryIndex;
use mcrs_minecraft_world::variant::{ChickenSoundVariant, ChickenVariant, ZombieNautilusVariant};
use mcrs_minecraft_worldgen::bevy::{
    StructureAsset, StructureSetAsset, TemplateAsset, TemplatePoolAsset,
};
use mcrs_minecraft_worldgen_feature::template::PaletteState;
use mcrs_minecraft_worldgen_generator::features::possible_biomes;
use mcrs_minecraft_worldgen_generator::structures::{
    StructureInputs, VariantInputs, freeze, live_sets, resolve_palette_state,
};
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, FrozenStructures};
use mcrs_minecraft_worldgen_structure::{Structure, TemplatePool};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Resource, Default, Clone)]
pub struct DimensionStructures(pub BTreeMap<ResourceLocation, Arc<DimensionStructureTables>>);

pub fn dimension_tables(
    frozen: Arc<FrozenStructures>,
    biomes: &DynRegistryIndex<Biome>,
    sources: &DimensionBiomeSources,
    named: impl Fn(&bevy_asset::Handle<Biome>) -> Option<ResourceLocation>,
) -> DimensionStructures {
    let mut tables = DimensionStructures::default();
    for (dimension, source) in &sources.0 {
        let mut mask = FixedBitSet::with_capacity(biomes.len() as usize);
        for id in possible_biomes(source, &named) {
            if let Some(index) = biomes.get(id.as_str()) {
                mask.insert(index as usize);
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
            build_dimension_structures.before(crate::world::enqueue_dim_spawns_from_preset),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_dimension_structures(
    mut commands: Commands,
    sources: Option<Res<DimensionBiomeSources>>,
    sets: Res<Assets<StructureSetAsset>>,
    structures: Res<Assets<StructureAsset>>,
    pools: Res<Assets<TemplatePoolAsset>>,
    templates: Res<Assets<TemplateAsset>>,
    asset_server: Res<AssetServer>,
    blocks: Res<Blocks>,
    biomes: Res<DynRegistryIndex<Biome>>,
    biome_tags: Res<DynTagRegistry<Biome>>,
    chickens: Res<Assets<ChickenVariant>>,
    chicken_sounds: Res<Assets<ChickenSoundVariant>>,
    zombie_nautiluses: Res<Assets<ZombieNautilusVariant>>,
) {
    let Some(sources) = sources else { return };

    let sets = registry_of(&sets, &asset_server, "worldgen/structure_set", |asset| {
        &asset.set
    });
    let structure_assets = registry_of(&structures, &asset_server, "worldgen/structure", |asset| {
        asset
    });
    let pool_assets = registry_of(&pools, &asset_server, "worldgen/template_pool", |asset| {
        asset
    });
    let template_handles: BTreeMap<ResourceLocation, Handle<TemplateAsset>> = pool_assets
        .values()
        .map(|asset| &asset.deps)
        .chain(structure_assets.values().map(|asset| &asset.deps))
        .flat_map(|deps| deps.templates.iter())
        .map(|(id, handle)| (id.clone(), handle.clone()))
        .collect();
    let structures: BTreeMap<ResourceLocation, Structure> = structure_assets
        .iter()
        .map(|(id, asset)| (id.clone(), asset.structure.clone()))
        .collect();
    let pools: BTreeMap<ResourceLocation, TemplatePool> = pool_assets
        .iter()
        .map(|(id, asset)| (id.clone(), asset.pool.clone()))
        .collect();

    let template = |id: &ResourceLocation| {
        let handle = template_handles.get(id)?;
        Some(Cow::Borrowed(&templates.get(handle)?.template))
    };
    let resolve = |state: &PaletteState| resolve_palette_state(&blocks.0, state);
    let chickens = registry_of(&chickens, &asset_server, "chicken_variant", |asset| {
        &asset.spawn_conditions
    });
    let chicken_sounds: Vec<ResourceLocation> = registry_of(
        &chicken_sounds,
        &asset_server,
        "chicken_sound_variant",
        |_| &(),
    )
    .into_keys()
    .collect();
    let zombie_nautiluses = registry_of(
        &zombie_nautiluses,
        &asset_server,
        "zombie_nautilus_variant",
        |asset| &asset.spawn_conditions,
    );
    let frozen = freeze(&StructureInputs {
        sets: &sets,
        structures: &structures,
        pools: &pools,
        template: &template,
        resolve: &resolve,
        biomes: &biomes,
        biome_tags: &biome_tags,
        variants: &VariantInputs {
            chickens: Some(&chickens),
            chicken_sounds: &chicken_sounds,
            zombie_nautiluses: Some(&zombie_nautiluses),
        },
    })
    .unwrap_or_else(|error| panic!("the structure registries do not resolve: {error}"));
    tracing::info!(
        sets = frozen.sets.len(),
        structures = frozen.structures.len(),
        pools = frozen.pools.len(),
        templates = frozen.templates.len(),
        "froze the structure registries"
    );
    commands.insert_resource(dimension_tables(
        Arc::new(frozen),
        &biomes,
        &sources,
        |handle| rl_from_asset_path(asset_server.get_path(handle.id())?.path(), "worldgen/biome"),
    ));
}
