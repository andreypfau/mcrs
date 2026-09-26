use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, Direction, SectionPos};
use mcrs_minecraft_light::block::LightRegistry;
use mcrs_minecraft_light::level::LightBounds;

use crate::colors::{LightColors, LightType};

/// Emission is at most 15 and every step costs at least one level, so light
/// from further away than this arrives at zero.
pub const REACH: i32 = 14;

pub const EAST_FACE: u8 = 1 << 0;
pub const UP_FACE: u8 = 1 << 1;
pub const SOUTH_FACE: u8 = 1 << 2;

/// The section plus a one-block apron, so the mesher can sample every face.
pub fn section_output(section: SectionPos) -> (BlockPos, i32) {
    let size = SectionPos::SIZE as i32;
    (
        BlockPos::new(
            section.x * size - 1,
            section.y * size - 1,
            section.z * size - 1,
        ),
        size + 2,
    )
}

pub struct Region {
    pub min: BlockPos,
    pub size: i32,
    pub blocks: Box<[VoxelId]>,
}

impl Region {
    pub fn new<'a>(
        output_min: BlockPos,
        output_size: i32,
        bounds: LightBounds,
        registry: &LightRegistry,
        cells: impl Fn(SectionPos) -> Option<&'a [u16; SectionPos::VOLUME]>,
    ) -> Region {
        let min = BlockPos::new(
            output_min.x - REACH,
            output_min.y - REACH,
            output_min.z - REACH,
        );
        let size = output_size + 2 * REACH;
        let max = BlockPos::new(min.x + size - 1, min.y + size - 1, min.z + size - 1);
        let mut region = Region {
            min,
            size,
            blocks: vec![VoxelId(0); (size * size * size) as usize].into_boxed_slice(),
        };

        let (low, high) = (SectionPos::from(min), SectionPos::from(max));
        let width = SectionPos::SIZE as i32;
        for sy in low.y..=high.y {
            for sz in low.z..=high.z {
                for sx in low.x..=high.x {
                    let section = SectionPos::new(sx, sy, sz);
                    let source = if sy < bounds.min_section_y || sy > bounds.max_section_y {
                        Err(registry.outside())
                    } else {
                        cells(section).ok_or(registry.unloaded())
                    };
                    let base = BlockPos::new(sx * width, sy * width, sz * width);
                    for y in base.y.max(min.y)..(base.y + width).min(max.y + 1) {
                        for z in base.z.max(min.z)..(base.z + width).min(max.z + 1) {
                            for x in base.x.max(min.x)..(base.x + width).min(max.x + 1) {
                                let block = match source {
                                    Ok(cells) => {
                                        let local =
                                            (x - base.x) | (z - base.z) << 4 | (y - base.y) << 8;
                                        VoxelId(cells[local as usize])
                                    }
                                    Err(filler) => filler,
                                };
                                let index = region.index(x - min.x, y - min.y, z - min.z);
                                region.blocks[index] = block;
                            }
                        }
                    }
                }
            }
        }
        region
    }

    #[inline]
    pub fn index(&self, x: i32, y: i32, z: i32) -> usize {
        (x + self.size * (z + self.size * y)) as usize
    }

    pub fn cell_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Cost to enter each cell, and which of its east, up and south faces light
/// may not cross. A face's bit lives on the cell at its lower coordinate.
pub struct EdgeCosts {
    pub entry: Box<[u8]>,
    pub veto: Box<[u8]>,
}

impl EdgeCosts {
    pub fn new(region: &Region, registry: &LightRegistry) -> EdgeCosts {
        let cells = region.cell_count();
        let size = region.size as usize;
        let mut entry = vec![0u8; cells];
        let mut veto = vec![0u8; cells];
        let mut record = |cell: usize, cost: u8| {
            debug_assert!(
                entry[cell] == 0 || entry[cell] == cost,
                "entering a block costs {} from one side and {cost} from another",
                entry[cell]
            );
            entry[cell] = cost;
        };

        for y in 0..size {
            for z in 0..size {
                for x in 0..size {
                    let here = x + size * (z + size * y);
                    let faces = [
                        (EAST_FACE, Direction::East, x + 1 < size, 1),
                        (UP_FACE, Direction::Up, y + 1 < size, size * size),
                        (SOUTH_FACE, Direction::South, z + 1 < size, size),
                    ];
                    for (bit, dir, inside, stride) in faces {
                        if !inside {
                            continue;
                        }
                        let there = here + stride;
                        let (a, b) = (region.blocks[here], region.blocks[there]);
                        let forward = registry.attenuation(a, b, dir);
                        let backward = registry.attenuation(b, a, dir.opposite());
                        debug_assert_eq!(
                            forward.is_none(),
                            backward.is_none(),
                            "the shape veto between {a:?} and {b:?} depends on direction"
                        );
                        match forward {
                            None => veto[here] |= bit,
                            Some(cost) => record(there, cost),
                        }
                        if let Some(cost) = backward {
                            record(here, cost);
                        }
                    }
                }
            }
        }

        EdgeCosts {
            entry: entry.into_boxed_slice(),
            veto: veto.into_boxed_slice(),
        }
    }
}

pub struct Palette {
    pub types: Vec<LightType>,
}

impl Palette {
    pub fn of(region: &Region, registry: &LightRegistry, colours: &LightColors) -> Palette {
        let mut present = [false; 256];
        for &id in &region.blocks {
            if !registry.emission(id).is_zero() {
                present[colours.light_type(id).0 as usize] = true;
            }
        }
        let types = (0..=u8::MAX)
            .filter(|&t| present[t as usize])
            .map(LightType)
            .collect();
        Palette { types }
    }

    pub fn lane(&self, t: LightType) -> Option<usize> {
        self.types.binary_search(&t).ok()
    }
}

/// One byte per type, in the narrowest of 1, 2, 4 or 8 bytes that holds the
/// palette, and whole 8-byte words beyond that, so no type spans two words.
pub fn lane_bytes(types: usize) -> usize {
    match types {
        0..=2 => types,
        3..=4 => 4,
        _ => types.div_ceil(8) * 8,
    }
}
