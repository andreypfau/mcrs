use super::corpus_ores::{one_biome_registry, ore_program, ore_tables};
use super::structures::frozen_shared;
use super::{
    biome_id, block_tags, blocks, build_beta_router, build_program_with, corpus_biomes,
    corpus_climate, corpus_features, one_step, test_registries,
};
use crate::modern_carvers::ModernCarverBlockIds;
use crate::stages::{ColumnGenerator, ColumnProgram, FillContext, dimension_y_sections};
use crate::staging::SectionData;
use crate::structures::index::{BiomeLookup, StructureIndex};
use crate::structures::live_sets;
use crate::{BetaCaveBlockIds, ColumnBiomes, ColumnBlocks, SurfaceStates};
use bevy_app::App;
use fixedbitset::FixedBitSet;
use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_heightmap::heightmap_predicates;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_structure::frozen::DimensionStructureTables;
use mcrs_voxel::ColumnPos;
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Column = Vec<Vec<VoxelId>>;

pub type Region = BTreeMap<ColumnPos, Column>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumer {
    BetaOre,
    ModernOre,
    Tree,
    Corpus,
    Structure {
        id: &'static str,
        biome: &'static str,
    },
}

const TREE_BIOME: &str = "minecraft:forest";

const TREE_FEATURE: &str = "minecraft:trees_birch_and_oak_leaf_litter";

const TREE_SEED: u64 = 4242;

const CORPUS_BIOME: &str = "minecraft:plains";

const CORPUS_SEED: u64 = 0xC0FFEE;

const STRUCTURE_SEED: u64 = 0x51A6E;

pub struct Dimension {
    pub ctx: FillContext,
    pub centre: ColumnPos,
}

impl Dimension {
    pub fn install(&self, app: &mut App) {
        app.insert_resource(self.ctx.clone());
        app.insert_resource(blocks().clone());
    }
}

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
    let biome = self::biome_id(test_registries(), biome_id);
    let mut mask = FixedBitSet::with_capacity(corpus_biomes().len());
    mask.insert(biome.index());
    let tables = DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live: live_sets(frozen, &mask),
    };
    let index = StructureIndex::new(
        Arc::new(tables),
        seed as i64,
        Arc::clone(ctx.router.as_ref().expect("a noise context")),
        BiomeLookup::Fixed(biome.number()),
        Some(ctx.predicates.clone()),
        Arc::clone(&ctx.features().expect("the dimension has a program").world),
        Arc::clone(corpus_climate()),
        -64,
        384,
    );
    let wanted = frozen.structure_ids[&ResourceLocationBuf::read(structure).unwrap()];
    let (pos, _) = index
        .locate(IVec3::ZERO, &[wanted])
        .unwrap_or_else(|| panic!("no {structure} within the search radius"));
    let centre = ColumnPos::new(pos.x >> 4, pos.z >> 4);
    assert!(
        !index.starts_reaching(centre).is_empty(),
        "{structure} at {centre:?} does not reach its own column"
    );
    ctx.structures = Some(Arc::new(index));
    Dimension { ctx, centre }
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
        return Dimension {
            ctx,
            centre: ColumnPos::new(0, 0),
        };
    }
    let router = Arc::new(build_beta_router());
    let y_sections = dimension_y_sections(&router, -64, 24);
    let (source, registry, tables) = match consumer {
        Consumer::BetaOre => {
            let (source, registry) = super::beta_surface::build_beta_biome_source();
            (Some(Arc::new(source)), registry, None)
        }
        Consumer::ModernOre => {
            let (tables, _) = ore_tables();
            (None, one_biome_registry(), Some(Arc::new(tables)))
        }
        Consumer::Tree | Consumer::Corpus | Consumer::Structure { .. } => {
            unreachable!("the feature and structure dimensions returned above")
        }
    };
    let program = match consumer {
        Consumer::BetaOre => ColumnProgram {
            generator: ColumnGenerator::Beta(Arc::new(BetaCaveBlockIds::resolve(&blocks().0))),
            biomes: ColumnBiomes::new(source.as_deref(), None, None).unwrap(),
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
                surface: Some((
                    crate::tests::surface_ids_over(&registry),
                    SurfaceStates::new(&blocks().0),
                )),
                carver_blocks: Arc::new(ModernCarverBlockIds::resolve(
                    &blocks().0,
                    block_tags().members(
                        block_tags()
                            .id(mcrs_minecraft_block::block_tag_keys::UNCARVABLE)
                            .expect("the corpus holds the uncarvable tag"),
                    ),
                )),
            },
            biomes: ColumnBiomes::new(source.as_deref(), None, None).unwrap(),
            carvers: None,
            features: tables.as_ref().map(|tables| Arc::new(ore_program(tables))),
        },
    };
    let ctx = FillContext {
        blocks: blocks().0.clone(),
        biome: source.clone(),
        predicates: heightmap_predicates(blocks(), block_tags()),
        program,
        router: Some(router),
        material: None,
        y_sections: y_sections.clone(),
        structures: None,
    };
    Dimension {
        ctx,
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
                    *cell = palette
                        .get_cell(index & 15, index >> 8, (index >> 4) & 15)
                        .unwrap();
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
