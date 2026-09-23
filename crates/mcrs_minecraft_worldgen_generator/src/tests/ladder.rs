//! The single-threaded oracle against the parallel scheduler.
//!
//! The definition of the stage leaves the scheduler free in which worker runs a
//! unit, when, in what order relative to units it shares no position with, and
//! whether a unit ran once or was re-run after an eviction. Every one of those
//! degrees of freedom is driven here, and every one must leave the decoded
//! blocks identical to the oracle's.

use super::corpus_ores::{one_biome_registry, ore_program, ore_tables};
use super::structures::frozen_shared;
use super::{
    biome_index, block_tags, blocks, build_beta_router, build_program_with, corpus_climate,
    corpus_features, one_step,
};
use crate::heightmap::heightmap_predicates;
use crate::modern_carvers::ModernCarverBlockIds;
use crate::saved::SectionData;
use crate::stages::{ColumnGenerator, ColumnProgram, FillContext, dimension_y_sections};
use crate::structures::index::{BiomeLookup, StructureIndex};
use crate::structures::live_sets;
use crate::{BetaCaveBlockIds, ColumnBlocks, SurfaceIds};
use bevy_app::App;
use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_worldgen_structure::frozen::DimensionStructureTables;
use std::collections::BTreeMap;
use std::sync::Arc;

/// One decoded column: its sections, each a flat block array in the packed
/// order of the palettes themselves.
pub type Column = Vec<Vec<VoxelId>>;

pub type Region = BTreeMap<ColumnPos, Column>;

/// Which program `Run` executes. Neither is the scheduler's business — it is
/// the same three stages either way — but a consumer that writes nothing would
/// leave the whole comparison two empty pipelines against each other, which is
/// what this file compared before either landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumer {
    /// Beta's populate step, which is `Run` for a beta dimension.
    BetaOre,
    /// The modifier machine over the corpus's own ore features.
    ModernOre,
    /// The corpus's own forest trees, whose crowns and decorators reach
    /// several blocks past the column that seeds them — the widest footprint
    /// any consumer has, and so the one the merge is really tested by.
    Tree,
    /// Every placed feature the corpus declares, over one biome: lakes, geodes,
    /// icebergs, dripstone clusters and huge fungi all writing into the same
    /// nine columns, which is a far wider and more varied set of deltas than
    /// any single family produces.
    Corpus,
    /// One structure over the region's centre in a dimension of one biome:
    /// pieces that straddle columns, each column reading the whole start and
    /// writing the pieces that cross it, block entities included.
    Structure {
        id: &'static str,
        biome: &'static str,
    },
}

/// The dimension the tree consumer runs in: the overworld router, forest
/// everywhere, and the biome's own `trees_birch_and_oak_leaf_litter`.
const TREE_BIOME: &str = "minecraft:forest";

const TREE_FEATURE: &str = "minecraft:trees_birch_and_oak_leaf_litter";

const TREE_SEED: u64 = 4242;

/// The dimension the corpus consumer runs in.
const CORPUS_BIOME: &str = "minecraft:plains";

const CORPUS_SEED: u64 = 0xC0FFEE;

/// The seed the structure consumers run at, in a dimension with no features,
/// so every write is a structure's.
const STRUCTURE_SEED: u64 = 0x51A6E;

/// One dimension, described once: the context the oracle drives the three stage
/// functions with, and the resources the dispatcher rebuilds that very context
/// from. The two must agree, or the comparison below is between two different
/// worlds rather than two orderings of one.
pub struct Dimension {
    pub ctx: FillContext,
    pub registry: Arc<RegistrySnapshot<Biome>>,
    /// The column the compared region is centred on.
    pub centre: ColumnPos,
}

impl Dimension {
    pub fn install(&self, app: &mut App) {
        app.insert_resource(self.ctx.clone());
        app.insert_resource(blocks().clone());
        app.insert_resource(RegistrySnapshot::clone(&self.registry));
        app.insert_resource(block_tags().clone());
        app.insert_resource(
            self.ctx
                .predicates
                .clone()
                .expect("the dimension carries the heightmap table"),
        );
    }
}

/// The overworld with one fixed biome and the shipped structure sets, centred
/// on the nearest start of `structure` the index finds from the origin.
pub(super) fn structure_dimension(structure: &str, biome_id: &str) -> Dimension {
    let frozen = frozen_shared();
    let seed = STRUCTURE_SEED;
    let (mut ctx, _) = super::trees::dimension_with(
        biome_id,
        |registry| {
            build_program_with(
                &one_step(vec![], biome_id),
                corpus_features(),
                registry,
                seed as i64,
                Some(frozen),
            )
        },
        seed,
    );
    let biome = biome_index()
        .get(biome_id)
        .expect("the biome index holds the corpus");
    let mut mask = FixedBitSet::with_capacity(biome_index().len() as usize);
    mask.insert(biome as usize);
    let tables = DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live: live_sets(frozen, &mask),
    };
    let index = StructureIndex::new(
        Arc::new(tables),
        seed as i64,
        Arc::clone(&ctx.router),
        BiomeLookup::Fixed(biome),
        ctx.predicates.clone(),
        Arc::clone(&ctx.features().expect("the dimension has a program").world),
        Arc::clone(corpus_climate()),
        -64,
        384,
    );
    let wanted = frozen.structure_ids[&ResourceLocation::parse(structure).unwrap()];
    let (pos, _) = index
        .locate(IVec3::ZERO, &[wanted])
        .unwrap_or_else(|| panic!("no {structure} within the search radius"));
    let centre = ColumnPos::new(pos.x >> 4, pos.z >> 4);
    assert!(
        !index.starts_reaching(centre).is_empty(),
        "{structure} at {centre:?} does not reach its own column"
    );
    ctx.structures = Some(Arc::new(index));
    let (_, registry) = ctx.biome.clone().expect("the dimension has a biome");
    Dimension {
        ctx,
        registry,
        centre,
    }
}

pub fn fill_context(consumer: Consumer) -> Dimension {
    if let Consumer::Structure { id, biome } = consumer {
        return structure_dimension(id, biome);
    }
    if let Consumer::Tree | Consumer::Corpus = consumer {
        let (ctx, _) = if consumer == Consumer::Tree {
            super::trees::tree_dimension(TREE_BIOME, TREE_FEATURE, TREE_SEED)
        } else {
            let tables = Arc::new(super::corpus_generators::every_placed_feature());
            super::trees::dimension_over(CORPUS_BIOME, tables, CORPUS_SEED)
        };
        let (_, registry) = ctx.biome.clone().expect("the dimension has a biome");
        return Dimension {
            ctx,
            registry,
            centre: ColumnPos::new(0, 0),
        };
    }
    let router = Arc::new(build_beta_router());
    let y_sections = dimension_y_sections(&router, -64, 24);
    let (source, registry, tables) = match consumer {
        Consumer::BetaOre => {
            let (source, registry) = super::beta_surface::build_beta_biome_source();
            (Some(Arc::new(source)), Arc::new(registry), None)
        }
        Consumer::ModernOre => {
            let (tables, _) = ore_tables();
            (None, Arc::new(one_biome_registry()), Some(Arc::new(tables)))
        }
        Consumer::Tree | Consumer::Corpus | Consumer::Structure { .. } => {
            unreachable!("the feature and structure dimensions returned above")
        }
    };
    let program = match consumer {
        Consumer::BetaOre => ColumnProgram {
            generator: ColumnGenerator::Beta(Arc::new(BetaCaveBlockIds::resolve(&blocks().0))),
            carvers: source
                .as_deref()
                .map(|source| Arc::new(super::beta_carver_table(source))),
            features: Some(Arc::new(super::beta_populate_program(
                &registry,
                router.world_seed as i64,
            ))),
        },
        _ => ColumnProgram {
            generator: ColumnGenerator::Modern {
                multi_noise: None,
                // Derived from the registry alone, exactly as the dispatcher derives
                // it. Neither consumer reaches the material surface — it needs a
                // biome grid, which only a multi-noise or fixed source builds — but
                // the context has to match all the same.
                surface: Some(Arc::new(SurfaceIds::resolve(&blocks().0, &registry))),
                carver_blocks: Arc::new(ModernCarverBlockIds::resolve(
                    &blocks().0,
                    Some(block_tags()),
                )),
            },
            carvers: None,
            features: tables.as_ref().map(|tables| Arc::new(ore_program(tables))),
        },
    };
    let ctx = FillContext {
        blocks: blocks().0.clone(),
        biome: source.clone().map(|src| (src, registry.clone())),
        // The modern vein probes `OCEAN_FLOOR_WG`, which is a terrain map, so
        // without the table it would place nothing at all.
        predicates: Some(heightmap_predicates(blocks(), block_tags())),
        saved: None,
        program,
        router,
        material: None,
        y_sections: y_sections.clone(),
        structures: None,
    };
    Dimension {
        ctx,
        registry,
        centre: ColumnPos::new(0, 0),
    }
}

pub fn decode(sections: &[Option<SectionData>]) -> Column {
    sections
        .iter()
        .map(|section| {
            let mut cells = vec![VoxelId::default(); ColumnBlocks::SECTION_VOLUME];
            if let Some((palette, _)) = section {
                for (index, cell) in cells.iter_mut().enumerate() {
                    *cell = palette.get_cell(index & 15, index >> 8, (index >> 4) & 15);
                }
            }
            cells
        })
        .collect()
}

pub fn region_columns(radius: i32) -> Vec<ColumnPos> {
    (-radius..=radius)
        .flat_map(|x| (-radius..=radius).map(move |z| ColumnPos::new(x, z)))
        .collect()
}
