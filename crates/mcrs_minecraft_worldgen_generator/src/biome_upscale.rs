use crate::multi_noise_biomes::BiomeGrid;
use mcrs_minecraft_biome::zoom::{FiddleCache, uniform_corners};
use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_core::{BlockPos, QuartPos, SectionPos};
use std::ops::Range;

const EDGE: usize = SectionPos::SIZE;
const CELLS: usize = EDGE / QuartPos::SIZE + 1;

struct ColumnBiomes<'a> {
    grid: &'a BiomeGrid,
    origin: QuartPos,
    first_row: i32,
    last_row: i32,
}

impl ColumnBiomes<'_> {
    fn at(&self, quart: QuartPos) -> u8 {
        let size = self.grid.volume.size();
        let x = quart.x - self.origin.x;
        let z = quart.z - self.origin.z;
        debug_assert!(
            (0..size.x).contains(&x) && (0..size.z).contains(&z),
            "quart {quart:?} lies outside the horizontal span of the grid"
        );
        let y = (quart.y.clamp(self.first_row, self.last_row) - self.origin.y).clamp(0, size.y - 1);
        self.grid.get(x, y, z)
    }

    fn single_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Option<u8> {
        let first = self.at(QuartPos::new(quart_x - 1, quart_y - 1, quart_z - 1));
        let span = CELLS as i32 + 1;
        (0..span)
            .all(|x| {
                (0..span).all(|z| {
                    (0..span).all(|y| {
                        self.at(QuartPos::new(
                            quart_x - 1 + x,
                            quart_y - 1 + y,
                            quart_z - 1 + z,
                        )) == first
                    })
                })
            })
            .then_some(first)
    }
}

fn cell_span(cell: usize) -> Range<usize> {
    (cell * QuartPos::SIZE).saturating_sub(2)..(cell * QuartPos::SIZE + 2).min(EDGE)
}

pub fn upscale_biomes(
    grid: &BiomeGrid,
    zoom_seed: i64,
    sections: &[i32],
    cache: &mut FiddleCache,
) -> Vec<VoxelPalette<u8, EDGE>> {
    let (Some(&first), Some(&last)) = (sections.first(), sections.last()) else {
        return Vec::new();
    };
    let min = grid.volume.min_block();
    let column = ColumnBiomes {
        grid,
        origin: QuartPos::of(min.into()),
        first_row: first * 4,
        last_row: last * 4 + 3,
    };
    let (block_x, block_z) = (min.x + 4, min.z + 4);

    cache.begin(
        zoom_seed,
        [
            (block_x - 2) >> 2,
            (first * 16 - 2) >> 2,
            (block_z - 2) >> 2,
        ],
        [6, (last - first + 1) * 4 + 2, 6],
    );

    sections
        .iter()
        .map(|&section_y| {
            let block_y = section_y * 16;
            if let Some(biome) = column.single_biome(block_x >> 2, section_y * 4, block_z >> 2) {
                return VoxelPalette::homogeneous(biome);
            }

            let mut cells = vec![0u8; EDGE * EDGE * EDGE];
            for cell_y in 0..CELLS {
                for cell_z in 0..CELLS {
                    for cell_x in 0..CELLS {
                        let (xs, ys, zs) =
                            (cell_span(cell_x), cell_span(cell_y), cell_span(cell_z));
                        let corner = BlockPos::new(
                            block_x + xs.start as i32,
                            block_y + ys.start as i32,
                            block_z + zs.start as i32,
                        );
                        let uniform = uniform_corners(corner, |quart| column.at(quart));
                        for y in ys {
                            for z in zs.clone() {
                                for x in xs.clone() {
                                    cells[(y * EDGE + z) * EDGE + x] =
                                        uniform.unwrap_or_else(|| {
                                            let pos = BlockPos::new(
                                                block_x + x as i32,
                                                block_y + y as i32,
                                                block_z + z as i32,
                                            );
                                            column.at(cache.quart_cell(pos))
                                        });
                                }
                            }
                        }
                    }
                }
            }
            VoxelPalette::from_cells(&cells)
        })
        .collect()
}
