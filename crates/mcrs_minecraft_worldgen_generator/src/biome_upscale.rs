use crate::multi_noise_biomes::BiomeGrid;
use mcrs_minecraft_biome::zoom::FiddleCache;
use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_core::{BlockPos, QuartPos, SectionPos};

const EDGE: usize = SectionPos::SIZE;

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
            let mut cells = vec![0u8; EDGE * EDGE * EDGE];
            for y in 0..EDGE {
                for z in 0..EDGE {
                    for x in 0..EDGE {
                        let pos = BlockPos::new(
                            block_x + x as i32,
                            block_y + y as i32,
                            block_z + z as i32,
                        );
                        cells[(y * EDGE + z) * EDGE + x] = column.at(cache.quart_cell(pos));
                    }
                }
            }
            VoxelPalette::from_cells(&cells)
        })
        .collect()
}
