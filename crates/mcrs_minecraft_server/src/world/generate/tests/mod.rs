mod base_height;
mod beta_biome_palette;
mod beta_cave_parity;
mod beta_ore_distribution;
pub(crate) mod beta_surface;
mod beta_surface_parity;
mod cell_census;
mod cell_fill;
mod corpus_generators;
mod corpus_ores;
mod footprint;
mod ladder;
mod modern_carvers;
mod modern_features;
mod multi_noise_biomes;
mod perf;
mod rungs;
mod structure_index;
mod structure_layouts;
mod structure_sites;
mod structures;
mod surface;
mod surface_parity;
mod template_manifest;
mod template_parity;
mod trees;

mod support;
pub use support::*;

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

use mcrs_minecraft_decoration::feature::terrain_skin::BiomeClimate;

use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_core::{RegistrySnapshot, ResourceLocation};
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_world::biome::Biome;
use mcrs_voxel_storage::{ColumnHeights, VoxelId};

use fixedbitset::FixedBitSet;
use mcrs_minecraft_worldgen::feature::compile::CompiledPlacedFeature;

use crate::world::chunk::{CancellationToken, ColumnSource};
use crate::world::generate::ColumnBlocks;
use crate::world::generate::feature_program::FeatureProgram;
use crate::world::generate::features::FeatureTables;
use crate::world::generate::stages::{FillContext, fill_column, merge_column, run_region};
use crate::world::generate::staging::{
    FilledSnapshot, RegionSnapshots, Stage, StagingStore, region_column,
};
use crate::world::heightmap::TerrainHeightmaps;
use mcrs_minecraft_worldgen::feature::compile::{FeatureSteps, LoadedFeatures};
use mcrs_minecraft_worldgen::feature::proto::{Feature, PlacedFeature};
use mcrs_minecraft_worldgen::structure::frozen::FrozenStructures;

/// Both feature registries of the shipped corpus, with every template and
/// processor list the features name, parsed once per test binary.
pub fn corpus_features() -> &'static LoadedFeatures {
    static CORPUS: LazyLock<LoadedFeatures> = LazyLock::new(|| {
        let features: BTreeMap<ResourceLocation, Feature> = load_json_dir("feature");
        let placed_features: BTreeMap<ResourceLocation, PlacedFeature> =
            load_json_dir("placed_feature");
        let mut templates = BTreeMap::new();
        for feature in features.values() {
            feature.for_each_feature(&mut |node| {
                let Feature::Template {
                    templates: entries, ..
                } = node
                else {
                    return;
                };
                for entry in entries {
                    templates.entry(entry.data.id.clone()).or_insert_with(|| {
                        structures::template_file(&entry.data.id)
                            .unwrap_or_else(|| {
                                panic!("{}: the template is not shipped", entry.data.id)
                            })
                            .into_owned()
                    });
                }
            });
        }
        LoadedFeatures {
            features,
            placed_features,
            templates,
            processor_lists: load_json_dir("processor_list"),
        }
    });
    &CORPUS
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
        biome_order: vec![ResourceLocation::parse(biome).unwrap()],
        climate: BTreeMap::from([(ResourceLocation::parse(biome).unwrap(), TEMPERATE)]),
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
    registry: &RegistrySnapshot<Biome>,
    seed: i64,
) -> FeatureProgram {
    build_program_with(tables, corpus, registry, seed, None)
}

pub fn build_program_with(
    tables: &FeatureTables,
    corpus: &LoadedFeatures,
    registry: &RegistrySnapshot<Biome>,
    seed: i64,
    structures: Option<&FrozenStructures>,
) -> FeatureProgram {
    let mut tables = tables.clone();
    for entry in registry.entries() {
        tables
            .climate
            .entry(entry.location.clone())
            .or_insert(TEMPERATE);
    }
    FeatureProgram::build(
        &tables,
        corpus,
        &blocks().0,
        Some(block_tags()),
        Some(fluid_tags()),
        registry,
        seed,
        structures,
    )
    .unwrap_or_else(|error| panic!("the feature program does not resolve: {error}"))
}

/// A fill context over one router and the corpus, with nothing else wired in.
pub fn bare_fill_context(
    router: impl Into<std::sync::Arc<mcrs_minecraft_worldgen::router::NoiseRouter>>,
) -> crate::world::generate::stages::FillContext {
    fill_context_with(router, None)
}

/// A context whose region can answer what a generator asks of the world.
///
/// A region reads its state sets off the program, so one built without it
/// answers every `isAir` and every sturdy-face question with the empty set —
/// which silently turns the modifiers that walk the ground into no-ops.
pub fn fill_context_with(
    router: impl Into<std::sync::Arc<mcrs_minecraft_worldgen::router::NoiseRouter>>,
    features: Option<std::sync::Arc<crate::world::generate::feature_program::FeatureProgram>>,
) -> crate::world::generate::stages::FillContext {
    let router = router.into();
    crate::world::generate::stages::FillContext {
        y_sections: crate::world::generate::stages::dimension_y_sections(&router, -64, 24),
        router,
        blocks: blocks().0.clone(),
        biome: None,
        predicates: None,
        saved: None,
        program: crate::world::generate::stages::ColumnProgram::modern(features),
        structures: None,
    }
}

/// A biome registry naming `names` in order, every one the Beta palette biome:
/// the feature tables only need the ids to resolve, and the surface stage only
/// needs its own three to exist.
pub fn biome_registry(names: &[&str]) -> RegistrySnapshot<Biome> {
    let mut assets = bevy_asset::Assets::<Biome>::default();
    let handle = assets.add(beta_biome_palette::make_beta_biome());
    RegistrySnapshot::<Biome>::build(
        names
            .iter()
            .map(|name| (ResourceLocation::parse(name).unwrap(), handle.id()))
            .collect::<Vec<_>>(),
        &assets,
        |_| Ok(mcrs_minecraft_nbt::compound::NbtCompound::new()),
    )
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
    source: &mcrs_minecraft_world::biome::source::BiomeSource,
) -> crate::world::generate::modern_carvers::CarverBiomeTable {
    crate::world::generate::modern_carvers::CarverBiomeTable::beta(source, |_| {
        Arc::from([mcrs_minecraft_worldgen::carver::CarverConfig::BetaCave])
    })
    .expect("a Beta biome source")
}

/// The program every biome of a Beta `registry` runs: the shipped populate step,
/// alone in its one step.
pub fn beta_populate_program(registry: &RegistrySnapshot<Biome>, seed: i64) -> FeatureProgram {
    let id = ResourceLocation::parse("minecraft:beta_populate").unwrap();
    let entry = Arc::new(CompiledPlacedFeature {
        placed: corpus_features().placed_features[&id].clone(),
        id: Some(id),
    });
    let mut carried = FixedBitSet::with_capacity(1);
    carried.insert(0);
    let biome_order: Vec<ResourceLocation> = registry
        .entries()
        .iter()
        .map(|entry| entry.location.clone())
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
    build_program(&tables, corpus_features(), registry, seed)
}
