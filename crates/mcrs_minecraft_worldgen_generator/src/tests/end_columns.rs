use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};

use glam::IVec3;

use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_heightmap::heightmap_predicates;
use mcrs_minecraft_registries::Diagnostics;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_voxel::ColumnPos;

use super::trees::tree_tables;
use super::{block_tags, blocks, build_program, corpus_features, generate_region};
use crate::ids::require_id;
use crate::modern_carvers::ModernCarverBlockIds;
use crate::stages::{ColumnGenerator, ColumnProgram, FillContext, dimension_y_sections};
use crate::structures::index::EndBiomes;
use crate::{ColumnBiomes, SurfaceStates, column_biome_palettes, end_biome_grid};

const IDS: EndBiomes = EndBiomes {
    end: 1,
    highlands: 2,
    midlands: 3,
    islands: 4,
    barrens: 5,
};

static ROUTER: LazyLock<NoiseRouter> = LazyLock::new(|| super::build_settings_router("end", 7));

#[test]
fn an_end_column_near_the_origin_stores_the_end_alone() {
    let biomes = ColumnBiomes::new(Some(&BiomeSource::TheEnd), None, Some(IDS)).unwrap();
    let y_sections = dimension_y_sections(&ROUTER, 0, 16);
    let (palettes, _) = column_biome_palettes(&ROUTER, &biomes, 0, 0, &y_sections);
    let mut stored = BTreeSet::new();
    for palette in &palettes {
        palette.for_each_distinct(|biome| {
            stored.insert(biome);
        });
    }
    assert_eq!(stored, BTreeSet::from([1]));
}

/// The first chunk out along x whose eight neighbours pick differently across its diagonal, so a
/// grid that swaps x and z or reads the centre of the wrong neighbour shows it.
fn chunk_on_a_biome_edge(ws: &mut Workspace) -> (i32, i32) {
    let mut pick = |x: i32, z: i32| IDS.at(&ROUTER, ws, IVec3::new(x * 4, 0, z * 4));
    (100..2000)
        .map(|chunk_x| (chunk_x, 37))
        .find(|&(x, z)| {
            [(-1, 0), (-1, 1), (0, 1)]
                .into_iter()
                .any(|(dx, dz)| pick(x + dx, z + dz) != pick(x + dz, z + dx))
        })
        .expect("the outer end has biome edges")
}

#[test]
fn every_quart_of_an_end_column_takes_the_biome_the_structures_pick() {
    let narrowed = IDS.try_map(u8::try_from).unwrap();
    let y_sections = dimension_y_sections(&ROUTER, 0, 16);
    let mut ws = Workspace::new();
    let edge = chunk_on_a_biome_edge(&mut ws);
    for (chunk_x, chunk_z) in [(0, 0), (64, 0), (0, -65), edge] {
        let grid = end_biome_grid(&ROUTER, &narrowed, chunk_x * 16, chunk_z * 16, &y_sections)
            .expect("an end column has rows");
        let size = grid.volume.size();
        for z in 0..size.z {
            for x in 0..size.x {
                for y in 0..size.y {
                    let block = IVec3::new(
                        grid.volume.block_x(x),
                        grid.volume.block_y(y),
                        grid.volume.block_z(z),
                    );
                    assert_eq!(
                        u16::from(grid.get(x, y, z)),
                        IDS.at(&ROUTER, &mut ws, block >> 2),
                        "chunk {chunk_x},{chunk_z}, block {block}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_end_spikes_stand_around_the_origin() {
    let seed = 7;
    let registry = super::biome_registry(&[
        "minecraft:badlands",
        "minecraft:eroded_badlands",
        "minecraft:frozen_ocean",
        "minecraft:deep_frozen_ocean",
        "minecraft:the_end",
        "minecraft:end_highlands",
        "minecraft:end_midlands",
        "minecraft:small_end_islands",
        "minecraft:end_barrens",
    ]);
    let mut diagnostics = Diagnostics::default();
    let end = EndBiomes::from_lookup(|key| require_id(&registry, key, &mut diagnostics))
        .unwrap_or_else(|| panic!("the registry holds the end biomes: {diagnostics}"));
    let (router, material) = super::material_router("end", seed, &registry);
    let router = Arc::new(router);
    let y_sections = dimension_y_sections(&router, 0, 16);
    let tables = tree_tables("minecraft:the_end", "minecraft:end_spike");
    let program = build_program(&tables, corpus_features(), &registry, seed as i64);
    let source = BiomeSource::TheEnd;
    let ctx = FillContext {
        blocks: blocks().0.clone(),
        biome: Some(Arc::new(source.clone())),
        predicates: Some(heightmap_predicates(blocks(), block_tags())),
        saved: None,
        program: ColumnProgram {
            generator: ColumnGenerator::Modern {
                surface: Some((
                    super::surface_ids_over(&registry),
                    SurfaceStates::new(&blocks().0),
                )),
                carver_blocks: Arc::new(ModernCarverBlockIds::for_test(Vec::new())),
            },
            biomes: ColumnBiomes::new(Some(&source), None, Some(end)).unwrap(),
            carvers: None,
            features: Some(Arc::new(program)),
        },
        router: Some(router),
        material: Some(Arc::new(material)),
        y_sections,
        structures: None,
    };
    let obsidian = VoxelId::from(
        blocks()
            .0
            .default_state("minecraft:obsidian")
            .unwrap()
            .number(),
    );

    let region = generate_region(&ctx, ColumnPos::new(-3, -3), ColumnPos::new(2, 2));

    let holds_obsidian = region.values().any(|column| {
        column.sections.iter().flatten().any(|(palette, _)| {
            let mut found = false;
            palette.for_each_distinct(|state| found |= state == obsidian);
            found
        })
    });
    assert!(
        holds_obsidian,
        "the spikes of the end stand within 48 blocks of the origin"
    );
}
