use crate::world::generate::routers::DimensionBiomeSources;
use bevy_app::{App, Plugin};
use bevy_asset::{Assets, Handle};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_state::prelude::OnEnter;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::parameter_list::{ParameterLists, parameter_lists_of};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_entity::keys::{CAT_VARIANT, CHICKEN_VARIANT, ZOMBIE_NAUTILUS_VARIANT};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Registry, RegistrySet};
use mcrs_minecraft_world::variant::spawn_selectors;
use mcrs_minecraft_worldgen::bevy::TemplateAsset;
use mcrs_minecraft_worldgen::tables::{WorldgenTables, named};
use mcrs_minecraft_worldgen_feature::template::PaletteState;
use mcrs_minecraft_worldgen_generator::features::possible_biomes;
use mcrs_minecraft_worldgen_generator::structures::{
    StructureInputs, VariantInputs, freeze, live_sets, resolve_palette_state,
};
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, FrozenStructures};
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet, TemplatePool};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Resource, Default, Clone)]
pub struct DimensionStructures(pub BTreeMap<ResourceLocation, Arc<DimensionStructureTables>>);

pub fn dimension_tables(
    frozen: Arc<FrozenStructures>,
    biomes: &Registry<Biome>,
    lists: &ParameterLists,
    sources: &DimensionBiomeSources,
) -> DimensionStructures {
    let mut tables = DimensionStructures::default();
    for (dimension, source) in &sources.0 {
        let mut mask = FixedBitSet::with_capacity(biomes.len());
        for name in possible_biomes(source, biomes, lists) {
            if let Some(id) = biomes.by_name(name.as_str()) {
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
            build_dimension_structures.before(crate::world::enqueue_dim_spawns),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_dimension_structures(
    mut commands: Commands,
    sources: Option<Res<DimensionBiomeSources>>,
    tables: Res<WorldgenTables>,
    templates: Res<Assets<TemplateAsset>>,
    blocks: Res<Blocks>,
    registries: Res<RegistrySet>,
) {
    let Some(sources) = sources else { return };
    let biomes = registries
        .registry::<Biome>()
        .expect("the data pack loader parses minecraft:worldgen/biome");
    let biome_tags = registries
        .tags::<Biome>()
        .expect("the data pack loader builds the biome tags");
    let structure_registry = registries
        .registry::<keys::Structure>()
        .expect("the data pack declares minecraft:worldgen/structure");
    let structure_tags = registries
        .tags::<keys::Structure>()
        .expect("the data pack loader builds the structure tags");

    let sets: BTreeMap<ResourceLocation, StructureSet> =
        named(&registries, &tables.structure_sets, |set| set)
            .into_iter()
            .map(|(id, set)| (id, set.clone()))
            .collect();
    let structure_assets = named(&registries, &tables.structures, |asset| asset);
    let pool_assets = named(&registries, &tables.template_pools, |asset| asset);
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
    let cats = spawn_selectors(&registries, CAT_VARIANT);
    let chickens = spawn_selectors(&registries, CHICKEN_VARIANT);
    let zombie_nautiluses = spawn_selectors(&registries, ZOMBIE_NAUTILUS_VARIANT);
    let names = |registry: &str| -> Vec<ResourceLocation> {
        registries
            .table(registry)
            .map(|table| table.names().to_vec())
            .unwrap_or_default()
    };
    let cat_sounds = names(
        mcrs_minecraft_entity::keys::CAT_SOUND_VARIANT
            .location()
            .as_static_str(),
    );
    let chicken_sounds = names(
        mcrs_minecraft_entity::keys::CHICKEN_SOUND_VARIANT
            .location()
            .as_static_str(),
    );
    let frozen = freeze(&StructureInputs {
        sets: &sets,
        structures: &structures,
        pools: &pools,
        template: &template,
        resolve: &resolve,
        biomes: &biomes,
        biome_tags: &biome_tags,
        structure_registry: &structure_registry,
        structure_tags: &structure_tags,
        variants: &VariantInputs {
            cats: Some(&cats),
            cat_sounds: &cat_sounds,
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
        &parameter_lists_of(&registries),
        &sources,
    ));
}
