use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_block::material::map::MapColor;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_generator::staging::FilledSnapshot;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos};

use crate::arguments::Slice;
use crate::game_data::GameData;
use crate::image::{Image, Rgba};

struct AreaBlock {
    column: u32,
    row: u32,
    x: i32,
    z: i32,
}

pub struct BiomeMap {
    pub image: Image,
    pub seen: BTreeSet<u8>,
}

pub struct AreaView<'a> {
    pub data: &'a GameData,
    pub columns: &'a BTreeMap<ColumnPos, FilledSnapshot>,
    pub min_x: i32,
    pub min_z: i32,
    pub width: u32,
    pub depth: u32,
}

impl AreaView<'_> {
    fn column(&self, x: i32, z: i32) -> Option<&FilledSnapshot> {
        self.columns.get(&ColumnPos::from(BlockPos::new(x, 0, z)))
    }

    fn surface(&self, x: i32, z: i32) -> Option<i32> {
        let maps = self.column(x, z)?.maps.as_ref()?;
        Some(maps.surface.0.get(local(x), local(z)))
    }

    fn block(&self, x: i32, y: i32, z: i32) -> Option<VoxelId> {
        let column = self.column(x, z)?;
        let (blocks, _) = column.sections.get(column.slot(y)?)?.as_ref()?;
        Some(blocks.get(local_pos(x, y, z)))
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        let column = self.column(x, z)?;
        let (_, biomes) = column.sections.get(column.slot(y)?)?.as_ref()?;
        Some(biomes.get(local_pos(x, y, z)))
    }

    fn block_color(&self, block: VoxelId) -> Option<Rgba> {
        let definitions = &self.data.blocks.0;
        let state = BlockStateId::checked(block.0, definitions.state_count())?;
        let color = definitions.state(state).map_color;
        let MapColor { r, g, b } = color;
        (color != MapColor::NONE).then(|| Rgba::opaque(r, g, b))
    }

    fn top_color(&self, x: i32, z: i32) -> Option<Rgba> {
        let top = self.surface(x, z)?;
        (self.data.min_y..top)
            .rev()
            .find_map(|y| self.block_color(self.block(x, y, z)?))
    }

    fn blocks_of_area(&self) -> impl Iterator<Item = AreaBlock> + use<'_> {
        (0..self.depth).flat_map(move |row| {
            (0..self.width).map(move |column| AreaBlock {
                column,
                row,
                x: self.min_x.saturating_add_unsigned(column),
                z: self.min_z.saturating_add_unsigned(row),
            })
        })
    }

    pub fn height_map(&self) -> Image {
        let (low, high) = self
            .blocks_of_area()
            .filter_map(|block| self.surface(block.x, block.z))
            .fold((i32::MAX, i32::MIN), |(low, high), height| {
                (low.min(height), high.max(height))
            });
        let range = i64::from(high) - i64::from(low);
        let mut image = Image::new(self.width, self.depth);
        for AreaBlock { column, row, x, z } in self.blocks_of_area() {
            if let Some(height) = self.surface(x, z) {
                let shade = (i64::from(height) - i64::from(low)) * 255 / range.max(1);
                let shade = u8::try_from(shade).unwrap_or(u8::MAX);
                image.set(column, row, Rgba::opaque(shade, shade, shade));
            }
        }
        image
    }

    pub fn top_block_map(&self) -> Image {
        let mut image = Image::new(self.width, self.depth);
        for AreaBlock { column, row, x, z } in self.blocks_of_area() {
            if let Some(color) = self.top_color(x, z) {
                image.set(column, row, color);
            }
        }
        image
    }

    pub fn biome_map(&self) -> BiomeMap {
        let mut image = Image::new(self.width, self.depth);
        let mut seen = BTreeSet::new();
        for AreaBlock { column, row, x, z } in self.blocks_of_area() {
            let y = self.surface(x, z).unwrap_or(self.data.min_y);
            let below = y.saturating_sub(1).max(self.data.min_y);
            if let Some(biome) = self.biome(x, below, z) {
                seen.insert(biome);
                image.set(column, row, self.biome_color(biome));
            }
        }
        BiomeMap { image, seen }
    }

    pub fn biome_name(&self, biome: u8) -> Option<&ResourceLocationBuf> {
        self.data.biome_names.get(usize::from(biome))
    }

    pub fn biome_color(&self, biome: u8) -> Rgba {
        self.biome_name(biome)
            .map_or(Rgba::CLEAR, |name| name_color(&name.to_string()))
    }

    pub fn slice(&self, slice: Slice) -> Image {
        let across = match slice {
            Slice::X(_) => self.depth,
            Slice::Z(_) => self.width,
        };
        let mut image = Image::new(across, self.data.height);
        let top = self
            .data
            .min_y
            .saturating_add_unsigned(self.data.height)
            .saturating_sub(1);
        for offset in 0..across {
            let (x, z) = match slice {
                Slice::X(x) => (x, self.min_z.saturating_add_unsigned(offset)),
                Slice::Z(z) => (self.min_x.saturating_add_unsigned(offset), z),
            };
            for row in 0..self.data.height {
                let y = top.saturating_sub_unsigned(row);
                if let Some(color) = self
                    .block(x, y, z)
                    .and_then(|block| self.block_color(block))
                {
                    image.set(offset, row, color);
                }
            }
        }
        image
    }
}

fn local(coordinate: i32) -> usize {
    usize::try_from(coordinate & 15).unwrap_or(0)
}

fn local_pos(x: i32, y: i32, z: i32) -> LocalPos {
    let nibble = |coordinate: i32| u8::try_from(coordinate & 15).unwrap_or(0);
    LocalPos::new(nibble(x), nibble(y), nibble(z))
}

// FNV-1a over the name: the same biome keeps its colour across runs and packs.
fn name_color(name: &str) -> Rgba {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    let [red, green, blue, _] = hash.to_be_bytes();
    Rgba::opaque(red, green, blue)
}
