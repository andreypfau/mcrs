use std::cell::Cell;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_voxel::SectionPos;

pub struct ColumnBlocks {
    y_sections: Vec<i32>,
    first_section_y: i32,
    present_floors: Vec<i32>,
    cells: Vec<Cell<VoxelId>>,
}

impl ColumnBlocks {
    pub const SECTION_VOLUME: usize = BlockPalette::VOLUME;

    pub fn new(y_sections: &[i32]) -> Self {
        let mut present_floors = Vec::new();
        fill_present_floors(y_sections, &mut present_floors);
        Self {
            y_sections: y_sections.to_vec(),
            first_section_y: y_sections.first().copied().unwrap_or(0),
            present_floors,
            cells: vec![Cell::new(VoxelId::default()); y_sections.len() * Self::SECTION_VOLUME],
        }
    }

    pub fn reset(&mut self, y_sections: &[i32]) {
        if self.y_sections != y_sections {
            self.y_sections.clear();
            self.y_sections.extend_from_slice(y_sections);
            self.first_section_y = y_sections.first().copied().unwrap_or(0);
            fill_present_floors(y_sections, &mut self.present_floors);
            self.cells.resize(
                y_sections.len() * Self::SECTION_VOLUME,
                Cell::new(VoxelId::default()),
            );
        }
        self.cells.fill(Cell::new(VoxelId::default()));
    }

    pub fn from_sections(
        sections: &[Option<(BlockPalette, BiomePalette)>],
        y_sections: &[i32],
    ) -> Self {
        let column = Self::new(y_sections);
        column.unpack(sections);
        column
    }

    pub fn unpack(&self, sections: &[Option<(BlockPalette, BiomePalette)>]) {
        for (index, section) in sections.iter().enumerate() {
            let Some((blocks, _)) = section else { continue };
            let cells = self.section_cells(index);
            let mut at = 0usize;
            blocks.0.for_each(|value| {
                cells[at].set(value);
                at += 1;
            });
        }
    }

    pub fn y_sections(&self) -> &[i32] {
        &self.y_sections
    }

    pub fn section_cells(&self, index: usize) -> &[Cell<VoxelId>] {
        &self.cells[index * Self::SECTION_VOLUME..][..Self::SECTION_VOLUME]
    }

    #[inline]
    pub fn section_index(&self, world_y: i32) -> Option<usize> {
        self.slot(world_y >> 4)
    }

    #[inline]
    pub fn set_in_section(&self, index: usize, x: i32, y: i32, z: i32, value: VoxelId) {
        self.section_cells(index)[(y as usize) * 256 + (z as usize) * 16 + x as usize].set(value);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "six box bounds beside the section index and value in a public signature"
    )]
    pub fn fill_box_in_section(
        &self,
        index: usize,
        x0: i32,
        x1: i32,
        y0: i32,
        y1: i32,
        z0: i32,
        z1: i32,
        value: VoxelId,
    ) {
        let cells = self.section_cells(index);
        let (x0, x1) = (x0.max(0) as usize, x1.min(16) as usize);
        let (y0, y1) = (y0.max(0) as usize, y1.min(16) as usize);
        let (z0, z1) = (z0.max(0) as usize, z1.min(16) as usize);
        for y in y0..y1 {
            for z in z0..z1 {
                for x in x0..x1 {
                    cells[y * 256 + z * 16 + x].set(value);
                }
            }
        }
    }

    #[must_use]
    pub fn present_bottom(&self, world_y: i32) -> i32 {
        self.slot(world_y >> 4)
            .and_then(|slot| self.present_floors.get(slot))
            .copied()
            .unwrap_or(world_y)
    }

    #[inline]
    fn slot(&self, section_y: i32) -> Option<usize> {
        let guess = section_y.wrapping_sub(self.first_section_y) as usize;
        if self.y_sections.get(guess) == Some(&section_y) {
            return Some(guess);
        }
        self.y_sections.iter().position(|&sy| sy == section_y)
    }

    #[inline]
    fn cell(&self, local_x: i32, world_y: i32, local_z: i32) -> Option<&Cell<VoxelId>> {
        if !(0..16).contains(&local_x) || !(0..16).contains(&local_z) {
            return None;
        }
        let slot = self.slot(world_y >> 4)?;
        let local_y = world_y & 0xF;
        Some(
            &self.cells[slot * Self::SECTION_VOLUME
                + (local_y as usize) * 256
                + (local_z as usize) * 16
                + local_x as usize],
        )
    }

    #[inline]
    pub fn get(&self, local_x: i32, world_y: i32, local_z: i32) -> Option<VoxelId> {
        self.cell(local_x, world_y, local_z).map(Cell::get)
    }

    #[inline]
    pub fn set(&self, local_x: i32, world_y: i32, local_z: i32, value: VoxelId) {
        if let Some(cell) = self.cell(local_x, world_y, local_z) {
            cell.set(value);
        }
    }

    #[must_use]
    pub fn run_bottom(
        &self,
        local_x: i32,
        local_z: i32,
        top: i32,
        min_y: i32,
        state: VoxelId,
    ) -> i32 {
        let Some(column) = column_index(local_x, local_z) else {
            return top;
        };
        let mut bottom = top;
        while bottom > min_y {
            let below = bottom - 1;
            let Some(cells) = self.column_cells(below >> 4, column) else {
                return bottom;
            };
            let section_bottom = (below & !0xF).max(min_y);
            for y in (section_bottom..=below).rev() {
                if cells.get((y & 0xF) as usize * 256).map(Cell::get) != Some(state) {
                    return y + 1;
                }
            }
            bottom = section_bottom;
        }
        bottom
    }

    pub fn highest_below(
        &self,
        local_x: i32,
        local_z: i32,
        above: i32,
        min_y: i32,
        matches: impl Fn(VoxelId) -> bool,
    ) -> Option<i32> {
        let column = column_index(local_x, local_z)?;
        let mut top = above.checked_sub(1)?;
        while top >= min_y {
            let section_bottom = (top & !0xF).max(min_y);
            if let Some(cells) = self.column_cells(top >> 4, column) {
                for y in (section_bottom..=top).rev() {
                    if cells
                        .get((y & 0xF) as usize * 256)
                        .is_some_and(|cell| matches(cell.get()))
                    {
                        return Some(y);
                    }
                }
            }
            top = section_bottom.checked_sub(1)?;
        }
        None
    }

    fn column_cells(&self, section_y: i32, column: usize) -> Option<&[Cell<VoxelId>]> {
        let slot = self.slot(section_y)?;
        self.cells
            .get(slot * Self::SECTION_VOLUME + column..(slot + 1) * Self::SECTION_VOLUME)
    }

    pub fn fill_strip(&self, local_x: i32, local_z: i32, bottom: i32, top: i32, value: VoxelId) {
        if !(0..16).contains(&local_x) || !(0..16).contains(&local_z) {
            return;
        }
        let column = (local_z as usize) * 16 + local_x as usize;
        let mut y = bottom;
        while y <= top {
            let section_top = (y | 0xF).min(top);
            if let Some(slot) = self.slot(y >> 4) {
                let base = slot * Self::SECTION_VOLUME + column;
                for local_y in (y & 0xF)..=(section_top & 0xF) {
                    if let Some(cell) = self.cells.get(base + (local_y as usize) * 256) {
                        cell.set(value);
                    }
                }
            }
            let Some(next) = section_top.checked_add(1) else {
                return;
            };
            y = next;
        }
    }

    pub fn block_palettes(&self) -> Vec<BlockPalette> {
        self.y_sections
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let cells = self.section_cells(index);
                let first = cells[0].get();
                if cells.iter().all(|cell| cell.get() == first) {
                    return BlockPalette::homogeneous(first);
                }
                let mut cube = Box::new(
                    [[[VoxelId::default(); SectionPos::SIZE]; SectionPos::SIZE]; SectionPos::SIZE],
                );
                for (slot, cell) in cube
                    .as_flattened_mut()
                    .as_flattened_mut()
                    .iter_mut()
                    .zip(cells)
                {
                    *slot = cell.get();
                }
                BlockPalette::from_cube(cube)
            })
            .collect()
    }

    pub fn into_sections(
        &self,
        biomes: &[BiomePalette],
    ) -> Vec<Option<(BlockPalette, BiomePalette)>> {
        self.block_palettes()
            .into_iter()
            .zip(biomes)
            .map(|(blocks, biomes)| Some((blocks, biomes.clone())))
            .collect()
    }

    pub fn write_back(&self, sections: &mut [Option<(BlockPalette, BiomePalette)>]) {
        for (section, palette) in sections.iter_mut().zip(self.block_palettes()) {
            if let Some((blocks, _)) = section {
                *blocks = palette;
            }
        }
    }
}

fn column_index(local_x: i32, local_z: i32) -> Option<usize> {
    ((0..16).contains(&local_x) && (0..16).contains(&local_z))
        .then(|| (local_z as usize) * 16 + local_x as usize)
}

fn fill_present_floors(y_sections: &[i32], floors: &mut Vec<i32>) {
    floors.clear();
    let mut floor = i32::MIN;
    let mut previous = None;
    for &section_y in y_sections {
        if previous.and_then(|below: i32| below.checked_add(1)) != Some(section_y) {
            floor = section_y.saturating_mul(16);
        }
        floors.push(floor);
        previous = Some(section_y);
    }
}
