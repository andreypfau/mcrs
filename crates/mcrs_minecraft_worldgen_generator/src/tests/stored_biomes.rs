use mcrs_minecraft_biome::zoom::{FiddleCache, obfuscate_seed, quart_cell};
use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_core::{BlockPos, SectionPos};

use super::build_settings_router;
use super::multi_noise_biomes::overworld_table;
use crate::biome_upscale::upscale_biomes;
use crate::multi_noise_biomes::BiomeGrid;
use crate::multi_noise_grid;
use crate::stored_biomes::{column_cell, stored_biome};

type Container = VoxelPalette<u8, { SectionPos::SIZE }>;

struct RealColumn {
    block_x: i32,
    sections: Vec<i32>,
    zoom_seed: i64,
    grid: BiomeGrid,
    containers: Vec<Container>,
}

fn real_column() -> RealColumn {
    let router = build_settings_router("overworld", 2);
    let (table, _) = overworld_table();
    let sections: Vec<i32> = (-4..20).collect();
    let zoom_seed = obfuscate_seed(2);

    for chunk_x in 0..64 {
        let block_x = chunk_x * 16;
        let grid = multi_noise_grid(&router, &table, block_x, 0, &sections)
            .expect("the multi-noise path builds a grid");
        let containers = upscale_biomes(&grid, zoom_seed, &sections, &mut FiddleCache::default());
        let mixed = containers.iter().any(|container| {
            let mut distinct = 0;
            container.for_each_distinct(|_| distinct += 1);
            distinct > 1
        });
        if mixed {
            return RealColumn {
                block_x,
                sections,
                zoom_seed,
                grid,
                containers,
            };
        }
    }
    panic!("no column with a section of several biomes");
}

#[test]
fn the_stored_read_equals_the_clamped_zoom_at_every_block() {
    let column = real_column();
    let first = column.sections[0];
    let min = column.grid.volume.min_block();
    let rows = column.grid.volume.size().y;
    let expected = |pos: BlockPos| {
        let quart = quart_cell(column.zoom_seed, pos);
        column.grid.get(
            quart.x - (min.x >> 2),
            (quart.y - (min.y >> 2)).clamp(0, rows - 1),
            quart.z - (min.z >> 2),
        )
    };

    let lowest = first * 16;
    let highest = (first + column.sections.len() as i32) * 16 - 1;
    for y in lowest..=highest {
        for z in 0..16 {
            for x in 0..16 {
                let pos = BlockPos::new(column.block_x + x, y, z);
                assert_eq!(
                    stored_biome(&column.containers, first, pos.x, pos.y, pos.z),
                    expected(pos),
                    "block {x},{y},{z} of the column at x {}",
                    column.block_x
                );
            }
        }
    }
}

#[test]
fn the_layers_outside_a_real_column_read_its_edge_layers() {
    let column = real_column();
    let first = column.sections[0];
    let lowest = first * 16;
    let highest = (first + column.sections.len() as i32) * 16 - 1;

    for z in 0..16 {
        for x in 0..16 {
            let at = |y| stored_biome(&column.containers, first, column.block_x + x, y, z);
            for offset in 1..=2 {
                assert_eq!(at(lowest - offset), at(lowest), "block {x},{z} below");
                assert_eq!(at(highest + offset), at(highest), "block {x},{z} above");
            }
        }
    }
}

#[test]
fn column_cell_clamps_a_height_outside_the_column() {
    assert_eq!(column_cell(-1, 3, -16), (0, 0));
    assert_eq!(column_cell(-1, 3, -1), (0, 15));
    assert_eq!(column_cell(-1, 3, 0), (1, 0));
    assert_eq!(column_cell(-1, 3, 21), (2, 5));
    assert_eq!(column_cell(-1, 3, 31), (2, 15));
    assert_eq!(column_cell(-1, 3, -5000), (0, 0));
    assert_eq!(column_cell(-1, 3, 5000), (2, 15));
    assert_eq!(column_cell(0, 0, 40), (0, 0));
}

#[test]
fn a_height_outside_the_column_reads_the_edge_layer() {
    const BOTTOM: u8 = 7;
    const TOP: u8 = 9;
    let mut lowest = Container::homogeneous(1);
    lowest.fill_box(0, 16, 0, 1, 0, 16, BOTTOM);
    let mut middle = Container::homogeneous(3);
    middle.set_cell(15, 0, 15, 4);
    let mut highest = Container::homogeneous(2);
    highest.fill_box(0, 16, 15, 16, 0, 16, TOP);
    let sections = [lowest, middle, highest];

    assert_eq!(stored_biome(&sections, -1, 3, -16, 3), BOTTOM);
    assert_eq!(stored_biome(&sections, -1, 3, -15, 3), 1);
    assert_eq!(stored_biome(&sections, -1, 3, -17, 3), BOTTOM);
    assert_eq!(stored_biome(&sections, -1, 3, -4000, 3), BOTTOM);
    assert_eq!(stored_biome(&sections, -1, 3, 31, 3), TOP);
    assert_eq!(stored_biome(&sections, -1, 3, 30, 3), 2);
    assert_eq!(stored_biome(&sections, -1, 3, 32, 3), TOP);
    assert_eq!(stored_biome(&sections, -1, 3, 4000, 3), TOP);
    assert_eq!(stored_biome(&sections, -1, -1, 0, -1), 4);
    assert_eq!(stored_biome(&[], 0, 0, 0, 0), 0);
}
