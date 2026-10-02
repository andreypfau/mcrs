use std::collections::HashMap;

use bevy_math::IVec3;
use mcrs_minecraft_biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_biome::zoom::{FiddleCache, obfuscate_seed, quart_cell};
use mcrs_minecraft_chunk::{PalettedContainer, VoxelPalette};
use mcrs_minecraft_core::{BlockPos, QuartPos};
use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;

use super::build_settings_router;
use crate::biome_upscale::upscale_biomes;
use crate::multi_noise_biomes::{BiomeGrid, MultiNoiseBiomeTable};
use crate::multi_noise_grid;

fn overworld_table() -> MultiNoiseBiomeTable {
    let mut ids: HashMap<String, u8> = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = ids.len() as u8;
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    let source = MultiNoiseBiomeSource {
        preset: Some(mcrs_minecraft_core::ResourceLocation::parse("minecraft:overworld").unwrap()),
        biomes: None,
    };
    MultiNoiseBiomeTable::resolve(&source, |biome| Some(ids[biome]))
        .expect("the overworld preset resolves")
}

#[test]
fn the_upscale_equals_the_plain_zoom_at_every_block() {
    let router = build_settings_router("overworld", 2);
    let table = overworld_table();
    let sections: Vec<i32> = (-4..20).collect();
    let zoom_seed = obfuscate_seed(2);

    let mut found = None;
    for chunk_x in 0..64 {
        let grid = multi_noise_grid(&router, &table, chunk_x * 16, 0, &sections)
            .expect("the multi-noise path builds a grid");
        let containers = upscale_biomes(&grid, zoom_seed, &sections, &mut FiddleCache::default());
        let mixed = containers.iter().any(|container| {
            let mut distinct = 0;
            container.for_each_distinct(|_| distinct += 1);
            distinct > 1
        });
        if mixed {
            found = Some((chunk_x, grid, containers));
            break;
        }
    }
    let (chunk_x, grid, containers) = found.expect("a column with a section of several biomes");
    assert_eq!(containers.len(), sections.len());

    let min = grid.volume.min_block();
    let rows = grid.volume.size().y;
    let expected = |quart: QuartPos| {
        let x = quart.x - (min.x >> 2);
        let z = quart.z - (min.z >> 2);
        let y = (quart.y - (min.y >> 2)).clamp(1, rows - 2);
        grid.get(x, y, z)
    };

    for (container, &section_y) in containers.iter().zip(&sections) {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let pos = BlockPos::new(chunk_x * 16 + x, section_y * 16 + y, z);
                    assert_eq!(
                        container.get_cell(x as usize, y as usize, z as usize),
                        expected(quart_cell(zoom_seed, pos)),
                        "block {x},{y},{z} of section {section_y} at chunk {chunk_x}"
                    );
                }
            }
        }
    }
}

type Container = VoxelPalette<u8, 16>;

fn grid_of(size: IVec3, min_block: IVec3, id: impl Fn(i32, i32, i32) -> u8) -> BiomeGrid {
    let volume = SampleGrid::new(size, min_block, IVec3::splat(4));
    let mut ids = vec![0u8; volume.len()];
    for x in 0..size.x {
        for y in 0..size.y {
            for z in 0..size.z {
                ids[volume.index_unchecked(x, y, z)] = id(x, y, z);
            }
        }
    }
    BiomeGrid { volume, ids }
}

fn ringed_grid(sections: &[i32], id: impl Fn(i32, i32, i32) -> u8) -> BiomeGrid {
    let rows = sections.len() as i32 * 4 + 2;
    grid_of(
        IVec3::new(6, rows, 6),
        IVec3::new(48 - 4, sections[0] * 16 - 4, -32 - 4),
        id,
    )
}

fn assert_equals_plain_zoom(
    grid: &BiomeGrid,
    zoom_seed: i64,
    sections: &[i32],
    containers: &[Container],
    biome: impl Fn(QuartPos) -> u8,
) {
    let min = grid.volume.min_block();
    assert_eq!(containers.len(), sections.len());
    for (container, &section_y) in containers.iter().zip(sections) {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let pos = BlockPos::new(min.x + 4 + x, section_y * 16 + y, min.z + 4 + z);
                    assert_eq!(
                        container.get_cell(x as usize, y as usize, z as usize),
                        biome(quart_cell(zoom_seed, pos)),
                        "block {x},{y},{z} of section {section_y}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_row_outside_the_column_is_answered_by_the_edge_row() {
    const MARGIN: u8 = 99;
    let sections = [-2, -1, 0, 1];
    let zoom_seed = obfuscate_seed(5);
    let grid = ringed_grid(&sections, |x, y, z| {
        if y == 0 || y == sections.len() as i32 * 4 + 1 {
            MARGIN
        } else {
            ((x + y + 2 * z) % 3) as u8
        }
    });
    let min = grid.volume.min_block();
    let rows = grid.volume.size().y;
    let read = |quart: QuartPos, low: i32, high: i32| {
        grid.get(
            quart.x - (min.x >> 2),
            (quart.y - (min.y >> 2)).clamp(low, high),
            quart.z - (min.z >> 2),
        )
    };

    let containers = upscale_biomes(&grid, zoom_seed, &sections, &mut FiddleCache::default());

    for container in &containers {
        container.for_each_distinct(|biome| assert_ne!(biome, MARGIN));
    }
    assert_equals_plain_zoom(&grid, zoom_seed, &sections, &containers, |quart| {
        read(quart, 1, rows - 2)
    });

    let reads_margin = (0..16).any(|y| {
        (0..16).any(|z| {
            (0..16).any(|x| {
                let pos = BlockPos::new(min.x + 4 + x, sections[0] * 16 + y, min.z + 4 + z);
                read(quart_cell(zoom_seed, pos), 0, rows - 1) == MARGIN
            })
        })
    });
    assert!(
        reads_margin,
        "the margin rows are never reached by the zoom"
    );
}

#[test]
fn a_uniform_neighbourhood_gives_a_single_value_container() {
    let sections = [0, 1, 2];
    let zoom_seed = obfuscate_seed(5);
    let rows = sections.len() as i32 * 4 + 2;

    for margin in [5, 6] {
        let grid = ringed_grid(
            &sections,
            |_, y, _| {
                if y == 0 || y == rows - 1 { margin } else { 5 }
            },
        );
        let containers = upscale_biomes(&grid, zoom_seed, &sections, &mut FiddleCache::default());
        assert_eq!(containers.len(), sections.len());
        for container in &containers {
            assert!(
                matches!(container.0, PalettedContainer::Homogeneous(5)),
                "margin {margin}"
            );
        }
    }
}

#[test]
fn a_uniform_cell_is_filled_with_its_corner_biome() {
    let sections = [0, 1];
    let zoom_seed = obfuscate_seed(11);
    let grid = ringed_grid(&sections, |x, _, _| if x < 3 { 1 } else { 2 });
    let min = grid.volume.min_block();
    let rows = grid.volume.size().y;

    let containers = upscale_biomes(&grid, zoom_seed, &sections, &mut FiddleCache::default());

    for container in &containers {
        assert!(matches!(container.0, PalettedContainer::Heterogeneous(_)));
    }
    assert_equals_plain_zoom(&grid, zoom_seed, &sections, &containers, |quart| {
        grid.get(
            quart.x - (min.x >> 2),
            (quart.y - (min.y >> 2)).clamp(1, rows - 2),
            quart.z - (min.z >> 2),
        )
    });
}

#[test]
fn a_single_row_grid_answers_every_height_from_that_row() {
    let sections = [-2, -1, 0, 1];
    let zoom_seed = obfuscate_seed(23);
    let grid = grid_of(
        IVec3::new(6, 1, 6),
        IVec3::new(48 - 4, 0, -32 - 4),
        |x, _, z| ((x + 2 * z) % 4) as u8,
    );
    let min = grid.volume.min_block();

    let containers = upscale_biomes(&grid, zoom_seed, &sections, &mut FiddleCache::default());

    for container in &containers {
        let mut distinct = 0;
        container.for_each_distinct(|_| distinct += 1);
        assert!(distinct > 1);
    }
    assert_equals_plain_zoom(&grid, zoom_seed, &sections, &containers, |quart| {
        grid.get(quart.x - (min.x >> 2), 0, quart.z - (min.z >> 2))
    });
}
