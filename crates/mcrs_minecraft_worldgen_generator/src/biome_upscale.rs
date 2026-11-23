use crate::biome_window::BiomeWindow;
use mcrs_minecraft_biome::zoom::FiddleCache;
use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_core::QuartPos;
use mcrs_voxel::{BlockPos, SectionPos};
use std::ops::Range;

const EDGE: usize = SectionPos::SIZE;
const CELLS: usize = EDGE / QuartPos::SIZE + 1;
const SPAN: usize = CELLS + 1;

struct ColumnBiomes<'a> {
    grid: BiomeWindow<'a>,
    origin: QuartPos,
    first_row: i32,
    last_row: i32,
}

impl ColumnBiomes<'_> {
    fn at(&self, quart: QuartPos) -> u8 {
        let size = self.grid.volume().size();
        let x = quart.x - self.origin.x;
        let z = quart.z - self.origin.z;
        debug_assert!(
            (0..size.x).contains(&x) && (0..size.z).contains(&z),
            "quart {quart:?} lies outside the horizontal span of the grid"
        );
        let y = (quart.y.clamp(self.first_row, self.last_row) - self.origin.y).clamp(0, size.y - 1);
        self.grid.get(x, y, z)
    }

    fn neighbourhood(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Neighbourhood {
        let mut quarts = [[[0; SPAN]; SPAN]; SPAN];
        for (x, plane) in (quart_x - 1..).zip(&mut quarts) {
            for (y, row) in (quart_y - 1..).zip(plane) {
                for (z, biome) in (quart_z - 1..).zip(row) {
                    *biome = self.at(QuartPos::new(x, y, z));
                }
            }
        }
        Neighbourhood(quarts)
    }
}

struct Neighbourhood([[[u8; SPAN]; SPAN]; SPAN]);

impl Neighbourhood {
    fn single_biome(&self) -> Option<u8> {
        let quarts = self.0.as_flattened().as_flattened();
        let [first, ..] = *quarts else {
            return None;
        };
        quarts.iter().all(|&biome| biome == first).then_some(first)
    }

    fn most_common(&self) -> u8 {
        let mut tally = [0u16; 256];
        for &biome in self.0.as_flattened().as_flattened() {
            if let Some(count) = tally.get_mut(usize::from(biome)) {
                *count = count.saturating_add(1);
            }
        }
        let mut common = 0;
        let mut most = 0;
        for (biome, &count) in (0..=u8::MAX).zip(&tally) {
            if count > most {
                common = biome;
                most = count;
            }
        }
        common
    }

    fn corners(&self, x: usize, y: usize, z: usize) -> Option<[u8; 8]> {
        let corner =
            |dx: usize, dy: usize, dz: usize| self.0.get(x + dx)?.get(y + dy)?.get(z + dz).copied();
        Some([
            corner(0, 0, 0)?,
            corner(0, 0, 1)?,
            corner(0, 1, 0)?,
            corner(0, 1, 1)?,
            corner(1, 0, 0)?,
            corner(1, 0, 1)?,
            corner(1, 1, 0)?,
            corner(1, 1, 1)?,
        ])
    }
}

fn cell_span(cell: usize) -> Range<usize> {
    (cell * QuartPos::SIZE).saturating_sub(2)..(cell * QuartPos::SIZE + 2).min(EDGE)
}

#[expect(
    clippy::excessive_nesting,
    reason = "fills the upscaled cells of each quart cell in y, z, x order"
)]
pub fn upscale_biomes(
    grid: BiomeWindow<'_>,
    zoom_seed: i64,
    sections: &[i32],
    cache: &mut FiddleCache,
) -> Vec<VoxelPalette<u8, EDGE>> {
    let (Some(&first), Some(&last)) = (sections.first(), sections.last()) else {
        return Vec::new();
    };
    let min = grid.volume().min_block();
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
            let quarts = column.neighbourhood(block_x >> 2, section_y * 4, block_z >> 2);
            if let Some(biome) = quarts.single_biome() {
                return VoxelPalette::homogeneous(biome);
            }

            let common = quarts.most_common();
            let mut cube = Box::new([[[common; EDGE]; EDGE]; EDGE]);
            let cells = cube.as_flattened_mut().as_flattened_mut();
            for cell_y in 0..CELLS {
                for cell_z in 0..CELLS {
                    for cell_x in 0..CELLS {
                        let (xs, ys, zs) =
                            (cell_span(cell_x), cell_span(cell_y), cell_span(cell_z));
                        let Some(corners) = quarts.corners(cell_x, cell_y, cell_z) else {
                            continue;
                        };
                        let [first, ..] = corners;
                        if corners.iter().all(|&biome| biome == first) {
                            if first != common {
                                for y in ys {
                                    for z in zs.clone() {
                                        let row = (y * EDGE + z) * EDGE;
                                        if let Some(span) =
                                            cells.get_mut(row + xs.start..row + xs.end)
                                        {
                                            span.fill(first);
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        let terms = cache.cell_terms(BlockPos::new(
                            block_x + xs.start as i32,
                            block_y + ys.start as i32,
                            block_z + zs.start as i32,
                        ));
                        for y in ys {
                            for z in zs.clone() {
                                for x in xs.clone() {
                                    let pos = BlockPos::new(
                                        block_x + x as i32,
                                        block_y + y as i32,
                                        block_z + z as i32,
                                    );
                                    cells[(y * EDGE + z) * EDGE + x] = terms.nearest(&corners, pos);
                                }
                            }
                        }
                    }
                }
            }
            VoxelPalette::from_cube(cube)
        })
        .collect()
}
