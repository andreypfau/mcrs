#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable,
    clippy::unwrap_used,
    reason = "test support reports a failure by panicking"
)]
#[cfg(test)]
mod base_height;
#[cfg(test)]
mod beta_biome_grid;
#[cfg(test)]
mod beta_cave_capture;
#[cfg(test)]
mod beta_ore_distribution;
pub(crate) mod beta_surface;
#[cfg(test)]
mod beta_surface_capture;
#[cfg(test)]
mod biome_parity;
#[cfg(test)]
mod biome_upscale;
#[cfg(test)]
mod carving_pins;
#[cfg(test)]
mod carving_regions;
#[cfg(test)]
mod cell_census;
#[cfg(test)]
mod cell_fill;
pub mod corpus_generators;
pub mod corpus_ores;
#[cfg(test)]
mod footprint;
#[cfg(test)]
mod ids;
pub mod ladder;
#[cfg(test)]
mod modern_carvers;
#[cfg(test)]
mod modern_features;
#[cfg(test)]
mod multi_noise_biomes;
#[cfg(test)]
mod rungs;
#[cfg(test)]
mod saved_regions;
#[cfg(test)]
mod stored_biomes;
#[cfg(test)]
mod structure_geometry;
#[cfg(test)]
mod structure_index;
#[cfg(test)]
mod structure_layouts;
#[cfg(test)]
mod structure_pieces;
#[cfg(test)]
mod structure_sites;
pub mod structures;
pub mod surface;
#[cfg(test)]
mod surface_parity;
#[cfg(test)]
mod surface_pins;
#[cfg(test)]
mod template_manifest;
#[cfg(test)]
mod template_parity;
#[cfg(test)]
mod template_pins;
pub mod trees;

mod support;
pub use support::*;

use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;

use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_chunk::{ColumnHeights, VoxelId};
use mcrs_minecraft_registries::Registries;
use mcrs_voxel::ColumnPos;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_worldgen_feature::compile::CompiledPlacedFeature;

use crate::ColumnBlocks;
use crate::feature_program::FeatureProgram;
use crate::features::FeatureTables;
use crate::stages::{FillContext, fill_column, merge_column, run_region};
use crate::staging::{
    FilledSnapshot, RegionSnapshots, Stage, StagingStore, TerrainHeightmaps, region_column,
};
use crate::task::{CancellationToken, ColumnSource};
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_worldgen_feature::compile::{FeatureSteps, LoadedFeatures};
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature};
use mcrs_minecraft_worldgen_structure::frozen::FrozenStructures;
use mcrs_minecraft_worldgen_testing::registry;

/// `feature`, then every feature written inline inside it, in pre-order.
pub fn for_each_feature(feature: &Feature, f: &mut dyn FnMut(&Feature)) {
    f(feature);
    feature.visit_placed_features(&mut |holder| {
        if let Holder::Inline(placed) = holder
            && let Holder::Inline(inner) = &placed.feature
        {
            for_each_feature(inner, f);
        }
    });
}

/// Both feature registries of the shipped corpus, with every template and
/// processor list the features name, parsed once per process.
pub fn corpus_features() -> &'static LoadedFeatures {
    static CORPUS: LazyLock<LoadedFeatures> = LazyLock::new(|| {
        let features: BTreeMap<ResourceLocationBuf, Feature> = registry("feature");
        let placed_features: BTreeMap<ResourceLocationBuf, PlacedFeature> =
            registry("placed_feature");
        let mut templates = BTreeMap::new();
        for feature in features.values() {
            for_each_feature(feature, &mut |node| {
                for id in node.templates() {
                    templates.entry(id.clone()).or_insert_with(|| {
                        structures::template_file(id)
                            .unwrap_or_else(|| panic!("{id}: the template is not shipped"))
                            .into_owned()
                    });
                }
            });
        }
        LoadedFeatures {
            features,
            placed_features,
            templates,
            processor_lists: registry("processor_list"),
            block_state_providers: registry("block_state_provider"),
            placed_feature_tags: placed_feature_tags(),
        }
    });
    &CORPUS
}

fn placed_feature_tags() -> BTreeMap<ResourceLocationBuf, Vec<ResourceLocationBuf>> {
    use mcrs_minecraft_registries::Namer;

    let registries = test_registries();
    let tags = registries
        .tags::<mcrs_minecraft_biome::PlacedFeature>()
        .expect("the loaded registries hold the placed feature tags");
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

/// One step that `biome` carries whole; the order is the caller's, not the
/// sort's, which is fine wherever only its being fixed matters.
pub fn one_step(entries: Vec<Arc<CompiledPlacedFeature>>, biome: &str) -> FeatureTables {
    let mut carried = FixedBitSet::with_capacity(entries.len());
    carried.insert_range(..);
    FeatureTables {
        features: FeatureSteps {
            token: vec![(0..entries.len()).collect()],
            steps: vec![entries],
            per_biome: vec![vec![carried]],
        },
        biome_order: vec![ResourceLocationBuf::read(biome).unwrap()],
        climate: BTreeMap::from([(ResourceLocationBuf::read(biome).unwrap(), TEMPERATE)]),
    }
}

/// A climate no pass ever ices or snows.
pub const TEMPERATE: BiomeClimate = BiomeClimate {
    base_temperature: 1.0,
    frozen: false,
    has_precipitation: false,
};

/// The program the freeze would build from `tables`, over the corpus blocks
/// and tags. A registry biome the tables give no climate is temperate.
pub fn build_program(
    tables: &FeatureTables,
    corpus: &LoadedFeatures,
    registries: &Registries,
    seed: i64,
) -> FeatureProgram {
    build_program_with(tables, corpus, registries, seed, None)
}

pub fn build_program_with(
    tables: &FeatureTables,
    corpus: &LoadedFeatures,
    registries: &Registries,
    seed: i64,
    structures: Option<&FrozenStructures>,
) -> FeatureProgram {
    try_build_program(tables, corpus, registries, seed, structures)
        .unwrap_or_else(|error| panic!("the feature program does not resolve: {error}"))
}

pub fn try_build_program(
    tables: &FeatureTables,
    corpus: &LoadedFeatures,
    registries: &Registries,
    seed: i64,
    structures: Option<&FrozenStructures>,
) -> Result<FeatureProgram, mcrs_minecraft_worldgen_feature::compile::FeatureCompileError> {
    let mut tables = tables.clone();
    for (_, name, _) in biomes_of(registries).iter() {
        tables.climate.entry(name.clone()).or_insert(TEMPERATE);
    }
    FeatureProgram::build(&tables, corpus, &blocks().0, registries, seed, structures)
}

pub fn surface_ids_over(registries: &Registries) -> std::sync::Arc<crate::SurfaceIds> {
    let mut diagnostics = mcrs_minecraft_registries::Diagnostics::default();
    let ids = crate::SurfaceIds::resolve(registries, &mut diagnostics)
        .unwrap_or_else(|| panic!("the registries hold the surface biomes: {diagnostics}"));
    std::sync::Arc::new(ids)
}

/// A fill context over one router and the corpus, with nothing else wired in.
pub fn bare_fill_context(
    router: impl Into<std::sync::Arc<mcrs_minecraft_worldgen_density::router::NoiseRouter>>,
) -> crate::stages::FillContext {
    fill_context_with(router, None)
}

/// A context whose region can answer what a generator asks of the world.
///
/// A region reads its state sets off the program, so one built without it
/// answers every `isAir` and every sturdy-face question with the empty set —
/// which silently turns the modifiers that walk the ground into no-ops.
pub fn fill_context_with(
    router: impl Into<std::sync::Arc<mcrs_minecraft_worldgen_density::router::NoiseRouter>>,
    features: Option<std::sync::Arc<crate::feature_program::FeatureProgram>>,
) -> crate::stages::FillContext {
    let router = router.into();
    crate::stages::FillContext {
        y_sections: crate::stages::dimension_y_sections(&router, -64, 24),
        router,
        material: None,
        blocks: blocks().0.clone(),
        biome: None,
        predicates: None,
        saved: None,
        program: crate::stages::ColumnProgram::modern(features),
        structures: None,
    }
}

/// The loaded registries with the biomes of `names` numbered first, in name order: the feature
/// tables only need the ids to resolve, and the surface stage only needs its own three to exist.
pub fn biome_registry(names: &[&str]) -> Registries {
    let mut names = names.to_vec();
    names.sort_unstable();
    registries_over(&names)
}

pub fn parameter_lists() -> &'static mcrs_minecraft_biome::parameter_list::ParameterLists {
    static LISTS: std::sync::OnceLock<mcrs_minecraft_biome::parameter_list::ParameterLists> =
        std::sync::OnceLock::new();
    LISTS.get_or_init(|| {
        mcrs_minecraft_biome::parameter_list::parameter_lists_of(test_registries())
            .expect("the loaded registries hold the parameter lists")
    })
}

pub fn preset_tables() -> &'static crate::multi_noise_biomes::PresetBiomeTables {
    static TABLES: LazyLock<crate::multi_noise_biomes::PresetBiomeTables> = LazyLock::new(|| {
        let mut diagnostics = mcrs_minecraft_registries::Diagnostics::default();
        crate::multi_noise_biomes::PresetBiomeTables::resolve(test_registries(), &mut diagnostics)
            .unwrap_or_else(|| panic!("the corpus holds every preset biome: {diagnostics}"))
    });
    &TABLES
}

pub fn parameter_list_id(
    name: impl std::fmt::Display,
) -> mcrs_minecraft_resource::Id<
    mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList,
> {
    let name = name.to_string();
    let key = ResourceKeyBuf::from_location(
        ResourceLocationBuf::read(&name).unwrap_or_else(|e| panic!("{name}: {e}")),
    );
    mcrs_minecraft_registries::Resolver::id(test_registries(), &key)
        .unwrap_or_else(|e| panic!("{e}"))
}

/// A filled column of one block per section — `None` leaves the section empty
/// — with both terrain maps at `top` and no final maps.
pub fn flat_snapshot(
    col: ColumnPos,
    y_sections: &Arc<[i32]>,
    section_block: impl Fn(i32) -> Option<VoxelId>,
    top: Option<i32>,
) -> FilledSnapshot {
    let terrain = top.map(|top| {
        let mut heights = ColumnHeights::new(y_sections.len() as u32 * 16, y_sections[0] * 16);
        for x in 0..16 {
            for z in 0..16 {
                heights.set(x, z, top);
            }
        }
        TerrainHeightmaps {
            surface: heights.clone(),
            solid: heights,
        }
    });
    FilledSnapshot {
        col,
        y_sections: y_sections.clone(),
        sections: y_sections
            .iter()
            .map(|&section_y| {
                let block = section_block(section_y)?;
                Some((
                    BlockPalette::homogeneous(block),
                    BiomePalette::homogeneous(0),
                ))
            })
            .collect(),
        terrain,
        maps: None,
        source: ColumnSource::Generated,
        block_entities: Vec::new(),
        entities: Vec::new(),
        ticks: Vec::new(),
    }
}

/// The nine snapshots of the 3×3 around `center`, each built by `snapshot`.
pub fn region_of(
    center: ColumnPos,
    snapshot: impl Fn(ColumnPos) -> FilledSnapshot,
) -> RegionSnapshots {
    std::array::from_fn(|slot| Arc::new(snapshot(region_column(center, slot))))
}

/// The single-threaded executor: fill the region and the shell every rung
/// derives, then climb the ladder one rung at a time, running and merging the
/// widest ring each rung still needs. It is the same three functions the
/// dispatcher drives, in one fixed order.
///
/// It is deliberately not an in-place sequential decorator: within a rung, a
/// unit that could read an earlier unit's writes would implement a different
/// world. Between rungs it is the opposite — a unit reads every other unit's
/// writes, which is what a rung is for.
pub fn generate_region(
    ctx: &FillContext,
    min: ColumnPos,
    max: ColumnPos,
) -> BTreeMap<ColumnPos, FilledSnapshot> {
    let y_sections = &ctx.y_sections;
    let cancel = CancellationToken::new();
    let mut store = StagingStore::default();
    let region = |halo: i32| {
        (min.x - halo..=max.x + halo)
            .flat_map(move |x| (min.z - halo..=max.z + halo).map(move |z| ColumnPos::new(x, z)))
    };

    let rungs = ctx.rungs();
    let mut buffer = ColumnBlocks::new(y_sections);
    for col in region(2 * rungs as i32) {
        let Some(snapshot) = fill_column(ctx, col, &mut buffer, &cancel) else {
            continue;
        };
        store.insert_filled(snapshot);
        store.set_stage(col, Stage::Filled);
    }
    for rung in 0..rungs {
        let reach = 2 * (rungs - 1 - rung) as i32;
        for col in region(reach + 1) {
            let Some(region) = store.region(col, rung) else {
                continue;
            };
            for (target, delta) in run_region(ctx, &region, rung) {
                store.push_delta(col, rung, target, delta);
            }
            store.set_stage(col, Stage::Ran(rung as u8));
        }
        for col in region(reach) {
            let deltas = store.deltas(col, rung);
            let Some(base) = store.base(col, rung).cloned() else {
                continue;
            };
            let merged = merge_column(
                &base,
                &deltas,
                ctx.predicates.as_ref(),
                ctx.features()
                    .map(|program| &program.world.has_block_entity),
            );
            store.insert_staged(col, rung, Arc::new(merged));
            store.set_stage(col, Stage::Merged(rung as u8));
        }
    }
    region(0)
        .filter_map(|col| Some((col, FilledSnapshot::clone(store.merged(col)?))))
        .collect()
}

/// Beta's carver table over `source`, every land biome carving with Beta's caves
/// the way the shipped Beta biomes do.
pub fn beta_carver_table(
    source: &mcrs_minecraft_biome::source::BiomeSource,
) -> crate::modern_carvers::CarverBiomeTable {
    crate::modern_carvers::CarverBiomeTable::beta(source, |_| {
        Arc::from([mcrs_minecraft_worldgen_carver::config::CarverConfig::BetaCave])
    })
    .expect("a Beta biome source")
}

/// The biomes `source` can place, as a mask over the loaded biome numbers.
pub fn possible_biome_mask(source: &mcrs_minecraft_biome::source::BiomeSource) -> FixedBitSet {
    let registries = test_registries();
    let mut mask = FixedBitSet::with_capacity(corpus_biomes().len());
    for name in crate::features::possible_biomes(source, registries, parameter_lists()) {
        mask.insert(biome_id(registries, name.as_str()).index());
    }
    mask
}

/// The program every biome of `registries` runs: the shipped populate step, alone in its one
/// step.
pub fn beta_populate_program(registries: &Registries, seed: i64) -> FeatureProgram {
    let id = ResourceLocationBuf::read("minecraft:beta_populate").unwrap();
    let entry = Arc::new(CompiledPlacedFeature {
        placed: corpus_features().placed_features[&id].clone(),
        id: Some(id),
    });
    let mut carried = FixedBitSet::with_capacity(1);
    carried.insert(0);
    let biome_order: Vec<ResourceLocationBuf> = biomes_of(registries)
        .iter()
        .map(|(_, name, _)| name.clone())
        .collect();
    let tables = FeatureTables {
        features: FeatureSteps {
            token: vec![vec![0]],
            steps: vec![vec![entry]],
            per_biome: vec![vec![carried]; biome_order.len()],
        },
        biome_order,
        climate: BTreeMap::new(),
    };
    build_program(&tables, corpus_features(), registries, seed)
}
