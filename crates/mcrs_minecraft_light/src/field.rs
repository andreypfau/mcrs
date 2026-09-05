//! Addressing is section-major (`section_index << 12 | local`) so that a step
//! to a neighbour is bit arithmetic in the common case and only touches the
//! slower path when it crosses a section boundary.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_math::{BlockPos, ChunkPos, Direction};
use mcrs_voxel_storage::{PalettedContainer, VoxelId};

use crate::SectionBlocks;
use crate::level::{LightLevel, LocalPos, SECTION_WIDTH};
use crate::region::BlockBox;
use crate::storage::LightStorage;

/// Index of a cell within a [`FieldLayout`].
pub type CellIndex = u32;

/// Hoist this out of any loop over a whole section: resolving the section
/// position from an index costs three divisions.
pub fn block_in(section: ChunkPos, local: LocalPos) -> BlockPos {
    BlockPos::new(
        section.x * SECTION_WIDTH + local.x() as i32,
        section.y * SECTION_WIDTH + local.y() as i32,
        section.z * SECTION_WIDTH + local.z() as i32,
    )
}

const LOCAL_BITS: u32 = 3 * BLOCKS::BITS as u32;
const LOCAL_MASK: u32 = (1 << LOCAL_BITS) - 1;

/// Shape of a working area, in sections.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldLayout {
    origin: ChunkPos,
    dim_x: i32,
    dim_y: i32,
    dim_z: i32,
}

impl FieldLayout {
    /// Smallest section-aligned layout covering `area`.
    pub fn covering(area: BlockBox) -> Self {
        let origin = ChunkPos::from(area.min);
        let far = ChunkPos::from(area.max);
        Self {
            origin,
            dim_x: far.x - origin.x + 1,
            dim_y: far.y - origin.y + 1,
            dim_z: far.z - origin.z + 1,
        }
    }

    pub fn section_count(&self) -> usize {
        (self.dim_x * self.dim_y * self.dim_z) as usize
    }

    /// The number of section columns the layout spans.
    pub fn column_count(&self) -> usize {
        (self.dim_x * self.dim_z) as usize
    }

    /// Every cell of a section shares this, so a per-column table is resolved
    /// once per section rather than once per cell.
    pub fn column_index(&self, section_index: usize) -> usize {
        section_index % self.column_count()
    }

    pub fn cell_count(&self) -> usize {
        self.section_count() * BLOCKS::VOLUME
    }

    fn section_stride_z(&self) -> i32 {
        self.dim_x
    }

    fn section_stride_y(&self) -> i32 {
        self.dim_x * self.dim_z
    }

    pub fn sections(&self) -> impl Iterator<Item = (usize, ChunkPos)> + '_ {
        (0..self.section_count()).map(move |i| (i, self.section_pos(i)))
    }

    pub fn section_pos(&self, section_index: usize) -> ChunkPos {
        let i = section_index as i32;
        let sx = i % self.dim_x;
        let sz = (i / self.dim_x) % self.dim_z;
        let sy = i / (self.dim_x * self.dim_z);
        ChunkPos::new(self.origin.x + sx, self.origin.y + sy, self.origin.z + sz)
    }

    /// The block-coordinate box this layout spans.
    pub fn block_bounds(&self) -> BlockBox {
        let far = ChunkPos::new(
            self.origin.x + self.dim_x - 1,
            self.origin.y + self.dim_y - 1,
            self.origin.z + self.dim_z - 1,
        );
        BlockBox::of_section(self.origin).union(BlockBox::of_section(far))
    }

    /// Index of the first cell of a section, to be combined with a [`LocalPos`].
    #[inline]
    pub fn section_base(&self, section_index: usize) -> CellIndex {
        (section_index as CellIndex) << LOCAL_BITS
    }

    /// Steps one cell in `dir`, or `None` when that leaves the working area.
    ///
    /// Leaving the area is safe to ignore: the area is built with a shell of
    /// cells that no edit in this batch can reach, so nothing outside it changes.
    #[inline]
    pub fn step(&self, index: CellIndex, dir: Direction) -> Option<CellIndex> {
        // The local index is `x | z << 4 | y << 8` and the section grid runs x
        // fastest then z then y, so an axis is fully described by where its
        // local coordinate sits and by how far one section of it moves the
        // index. `Direction::id()` carries the axis above the sign bit, in the
        // order Y, Z, X.
        let (shift, section_stride, dim) = match dir.id() >> 1 {
            0 => (8, self.section_stride_y(), self.dim_y),
            1 => (4, self.section_stride_z(), self.dim_z),
            _ => (0, 1, self.dim_x),
        };
        let forward = dir.id() & 1 == 1;
        let cell = 1i32 << shift;

        let local = ((index >> shift) & 15) as i32;
        if local != if forward { 15 } else { 0 } {
            return Some((index as i32 + if forward { cell } else { -cell }) as CellIndex);
        }

        // Only the section-crossing path pays for the division.
        let coord = (index >> LOCAL_BITS) as i32 / section_stride % dim;
        let next = coord + if forward { 1 } else { -1 };
        if next < 0 || next >= dim {
            return None;
        }
        let jump = (section_stride << LOCAL_BITS) - 15 * cell;
        Some((index as i32 + if forward { jump } else { -jump }) as CellIndex)
    }
}

/// Immutable snapshot of the blocks under a working area.
///
/// Taking a snapshot is what makes the calculation safe to run off the tick
/// loop: the main thread may keep editing the world, and a section it edits
/// simply gets a fresh allocation while this one keeps reading the old.
pub struct BlockSnapshot {
    sections: Vec<SectionSource>,
}

/// Where one section's blocks come from.
pub enum SectionSource {
    Loaded(Arc<SectionBlocks>),
    /// Above or below the world. There are no blocks here, light passes, and
    /// this is where sky light enters from.
    Open(VoxelId),
    /// Inside the world but not loaded. Holds no light of its own and grows
    /// none: seeding it would let light escape, because leaving a block costs
    /// nothing and only entering one does.
    Absent(VoxelId),
}

impl SectionSource {
    #[inline]
    pub fn block_at(&self, local: LocalPos) -> VoxelId {
        match self {
            SectionSource::Loaded(blocks) => {
                blocks.get_cell(local.x() as usize, local.y() as usize, local.z() as usize)
            }
            SectionSource::Open(block) | SectionSource::Absent(block) => *block,
        }
    }

    /// The block this section holds everywhere, when it holds only one.
    pub fn uniform_block(&self) -> Option<VoxelId> {
        match self {
            SectionSource::Loaded(blocks) => match &blocks.0 {
                PalettedContainer::Homogeneous(block) => Some(*block),
                PalettedContainer::Heterogeneous(_) => None,
            },
            SectionSource::Open(block) | SectionSource::Absent(block) => Some(*block),
        }
    }
}

impl BlockSnapshot {
    pub fn new(layout: &FieldLayout, sections: Vec<SectionSource>) -> Self {
        debug_assert_eq!(sections.len(), layout.section_count());
        Self { sections }
    }

    /// Resolved once per section by the seeding pass, which would otherwise
    /// re-index and re-match for each of the section's 4096 cells.
    pub fn section(&self, section_index: usize) -> &SectionSource {
        &self.sections[section_index]
    }

    #[inline]
    pub fn get(&self, index: CellIndex) -> VoxelId {
        self.sections[(index >> LOCAL_BITS) as usize]
            .block_at(LocalPos::from_index((index & LOCAL_MASK) as usize))
    }

    pub fn section_loaded(&self, section_index: usize) -> bool {
        matches!(self.sections[section_index], SectionSource::Loaded(_))
    }

    /// Whether this section may hold light at all.
    pub fn section_holds_light(&self, section_index: usize) -> bool {
        !matches!(self.sections[section_index], SectionSource::Absent(_))
    }
}

/// Light values of a working area, one byte per cell.
///
/// The cells are atomic because the calculation writes them from many threads
/// at once with no partitioning. That is sound because every write is a
/// `fetch_max`: the result never depends on which thread got there first.
pub struct LightField {
    layout: FieldLayout,
    cells: Vec<AtomicU8>,
}

impl LightField {
    pub fn new(layout: FieldLayout) -> Self {
        // `AtomicU8` is `repr(transparent)` over `u8`, and a zeroed byte is a
        // zeroed atomic, so this reaches `alloc_zeroed` instead of writing
        // several megabytes of zeros per epoch.
        let zeros = vec![0u8; layout.cell_count()];
        let mut zeros = std::mem::ManuallyDrop::new(zeros);
        let cells = unsafe {
            Vec::from_raw_parts(
                zeros.as_mut_ptr() as *mut AtomicU8,
                zeros.len(),
                zeros.capacity(),
            )
        };
        Self { layout, cells }
    }

    pub fn layout(&self) -> &FieldLayout {
        &self.layout
    }

    #[inline]
    pub fn get(&self, index: CellIndex) -> LightLevel {
        LightLevel::new(self.cells[index as usize].load(Ordering::Relaxed))
    }

    #[inline]
    pub fn set(&self, index: CellIndex, level: LightLevel) {
        self.cells[index as usize].store(level.get(), Ordering::Relaxed);
    }

    /// Raises a cell to at least `level`, reporting whether it actually rose.
    ///
    /// The caller that sees `true` owns the obligation to spread the new value.
    /// A caller that read a stale, lower value elsewhere can only ever make a
    /// weaker claim, which some later write corrects.
    #[inline]
    pub fn raise(&self, index: CellIndex, level: LightLevel) -> bool {
        self.cells[index as usize].fetch_max(level.get(), Ordering::Relaxed) < level.get()
    }

    pub fn fill_section(&mut self, section_index: usize, light: &LightStorage) {
        // A fresh field is already zeroed, so an empty section needs no work.
        if matches!(light, LightStorage::Empty) {
            return;
        }
        let base = section_index * BLOCKS::VOLUME;
        let mut cells = [0u8; BLOCKS::VOLUME];
        light.write_field(&mut cells);
        for (cell, value) in self.cells[base..base + BLOCKS::VOLUME]
            .iter_mut()
            .zip(cells)
        {
            *cell.get_mut() = value;
        }
    }

    pub fn section_light(&self, section_index: usize) -> LightStorage {
        let base = section_index * BLOCKS::VOLUME;
        let mut cells = [0u8; BLOCKS::VOLUME];
        for (value, cell) in cells
            .iter_mut()
            .zip(&self.cells[base..base + BLOCKS::VOLUME])
        {
            *value = cell.load(Ordering::Relaxed);
        }
        LightStorage::from_field(&cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> FieldLayout {
        FieldLayout::covering(BlockBox {
            min: BlockPos::new(0, 0, 0),
            max: BlockPos::new(47, 47, 47),
        })
    }

    /// A step must move the cell exactly one block along the axis its direction
    /// names — the local packing and the section grid disagree on axis order,
    /// so this is not self-evident from either.
    #[test]
    fn a_step_moves_one_block_along_its_own_axis() {
        let layout = layout();
        let start = BlockPos::new(20, 20, 20);
        let index = index_of(&layout, start);
        for dir in Direction::all() {
            let stepped = layout.step(index, dir).expect("inside the area");
            let expected = BlockPos::new(
                start.x + dir.normal().x,
                start.y + dir.normal().y,
                start.z + dir.normal().z,
            );
            assert_eq!(block_of(&layout, stepped), expected, "{dir:?}");
        }
    }

    #[test]
    fn a_step_crossing_a_section_seam_lands_in_the_next_section() {
        let layout = layout();
        for (start, dir) in [
            (BlockPos::new(15, 20, 20), Direction::East),
            (BlockPos::new(16, 20, 20), Direction::West),
            (BlockPos::new(20, 15, 20), Direction::Up),
            (BlockPos::new(20, 16, 20), Direction::Down),
            (BlockPos::new(20, 20, 15), Direction::South),
            (BlockPos::new(20, 20, 16), Direction::North),
        ] {
            let stepped = layout
                .step(index_of(&layout, start), dir)
                .expect("inside the area");
            let expected = BlockPos::new(
                start.x + dir.normal().x,
                start.y + dir.normal().y,
                start.z + dir.normal().z,
            );
            assert_eq!(block_of(&layout, stepped), expected, "{start:?} {dir:?}");
        }
    }

    #[test]
    fn a_step_off_the_area_has_no_cell() {
        let layout = layout();
        assert_eq!(
            layout.step(index_of(&layout, BlockPos::new(0, 20, 20)), Direction::West),
            None
        );
        assert_eq!(
            layout.step(index_of(&layout, BlockPos::new(20, 47, 20)), Direction::Up),
            None
        );
        assert_eq!(
            layout.step(
                index_of(&layout, BlockPos::new(20, 20, 0)),
                Direction::North
            ),
            None
        );
    }

    fn index_of(layout: &FieldLayout, pos: BlockPos) -> CellIndex {
        let section = ChunkPos::from(pos);
        let index = (0..layout.section_count())
            .find(|&i| layout.section_pos(i) == section)
            .expect("section is in the area");
        layout.section_base(index)
            | LocalPos::new((pos.x & 15) as u8, (pos.y & 15) as u8, (pos.z & 15) as u8).index()
                as CellIndex
    }

    fn block_of(layout: &FieldLayout, index: CellIndex) -> BlockPos {
        block_in(
            layout.section_pos((index >> LOCAL_BITS) as usize),
            LocalPos::from_index((index & LOCAL_MASK) as usize),
        )
    }
}
