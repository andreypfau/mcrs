use std::cell::Cell;

use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;

/// Every block of one column, dense, in the section palette's own index order:
/// section, then y, then z, then x.
///
/// Generation writes each block several times — terrain, then surface, caves and
/// ores — and a paletted section pays a reverse-index lookup per write. Holding
/// the column dense makes those writes plain stores; the palettes are built once,
/// at the end, by a single scan per section.
///
/// Cells are interior-mutable so a reader and a writer closure can be handed to
/// the carver and the ore feature at the same time.
pub struct ColumnBlocks {
    y_sections: Vec<i32>,
    first_section_y: i32,
    cells: Vec<Cell<VoxelId>>,
}

impl ColumnBlocks {
    pub const SECTION_VOLUME: usize = BlockPalette::VOLUME;


    pub fn new(y_sections: &[i32]) -> Self {
        Self {
            y_sections: y_sections.to_vec(),
            first_section_y: y_sections.first().copied().unwrap_or(0),
            cells: vec![Cell::new(VoxelId::default()); y_sections.len() * Self::SECTION_VOLUME],
        }
    }

    /// Point this buffer at a new column, reusing the allocation.
    ///
    /// Generation runs column after column on the same worker, and a fresh
    /// buffer per column costs an allocation of a hundred kilobytes or more.
    pub fn reset(&mut self, y_sections: &[i32]) {
        if self.y_sections != y_sections {
            self.y_sections.clear();
            self.y_sections.extend_from_slice(y_sections);
            self.first_section_y = y_sections.first().copied().unwrap_or(0);
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
        for (index, section) in sections.iter().enumerate() {
            let Some((blocks, _)) = section else { continue };
            let cells = &column.cells[index * Self::SECTION_VOLUME..][..Self::SECTION_VOLUME];
            for y in 0..16i32 {
                for z in 0..16i32 {
                    for x in 0..16i32 {
                        cells[(y as usize) * 256 + (z as usize) * 16 + x as usize]
                            .set(blocks.get(BlockPos::new(x, y, z)));
                    }
                }
            }
        }
        column
    }

    pub fn y_sections(&self) -> &[i32] {
        &self.y_sections
    }

    /// The section's cells, in palette index order.
    pub fn section_cells(&self, index: usize) -> &[Cell<VoxelId>] {
        &self.cells[index * Self::SECTION_VOLUME..][..Self::SECTION_VOLUME]
    }

    /// Which slot holds the section containing `world_y`, if the column covers it.
    #[inline]
    pub fn section_index(&self, world_y: i32) -> Option<usize> {
        self.slot(world_y >> 4)
    }

    /// Section-local write, the dense counterpart of `BlockPalette::set`.
    #[inline]
    pub fn set_in_section(&self, index: usize, x: i32, y: i32, z: i32, value: VoxelId) {
        self.section_cells(index)[(y as usize) * 256 + (z as usize) * 16 + x as usize].set(value);
    }

    /// Section-local box fill, the dense counterpart of `BlockPalette::fill_box`.
    /// The box is clipped to the section, which is what the paletted container
    /// required of its callers anyway.
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

    /// Which slot holds `section_y`, if the column covers it. Sections are
    /// contiguous and ascending in practice, which the subtraction answers
    /// directly; the search is the fallback for a column with holes.
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

    /// `None` when the position falls outside the column.
    #[inline]
    pub fn get(&self, local_x: i32, world_y: i32, local_z: i32) -> Option<VoxelId> {
        self.cell(local_x, world_y, local_z).map(Cell::get)
    }

    /// Writes outside the column are dropped, as they were when the passes wrote
    /// through a section list that did not cover them.
    #[inline]
    pub fn set(&self, local_x: i32, world_y: i32, local_z: i32, value: VoxelId) {
        if let Some(cell) = self.cell(local_x, world_y, local_z) {
            cell.set(value);
        }
    }

    /// Build one palette per section, each from a single scan of its cells.
    ///
    /// Most sections of a tall column hold one value — air above the terrain,
    /// stone below it — and answering those with a scan that stops at the first
    /// difference keeps the copy and the palette build off that path entirely.
    pub fn block_palettes(&self) -> Vec<BlockPalette> {
        let mut scratch = [VoxelId::default(); Self::SECTION_VOLUME];
        self.y_sections
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let cells = self.section_cells(index);
                let first = cells[0].get();
                if cells.iter().all(|cell| cell.get() == first) {
                    return BlockPalette::homogeneous(first);
                }
                for (slot, cell) in scratch.iter_mut().zip(cells) {
                    *slot = cell.get();
                }
                BlockPalette::from_cells(&scratch)
            })
            .collect()
    }
}
