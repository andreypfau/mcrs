use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use mcrs_minecraft_chunk::{PalettedContainer, VoxelId};
use mcrs_minecraft_core::Direction;
use mcrs_voxel::{BlockPos, SectionPos};

use crate::SectionBlocks;
use crate::block::LightRegistry;
use crate::level::{LightLevel, SECTION_WIDTH};
use mcrs_minecraft_chunk::LightStorage;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_volume::{SectionBox, Snapshot, Unavailable};
use mcrs_voxel::LocalPos;

pub type CellIndex = u32;

pub fn block_in(section: SectionPos, local: LocalPos) -> BlockPos {
    BlockPos::new(
        section.x * SECTION_WIDTH + local.x() as i32,
        section.y * SECTION_WIDTH + local.y() as i32,
        section.z * SECTION_WIDTH + local.z() as i32,
    )
}

const LOCAL_BITS: u32 = 3 * SectionPos::BITS as u32;
const LOCAL_MASK: u32 = (1 << LOCAL_BITS) - 1;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldLayout {
    origin: SectionPos,
    dim_x: i32,
    dim_y: i32,
    dim_z: i32,
}

impl FieldLayout {
    pub fn covering(area: BoundingBox) -> Self {
        let origin = SectionPos::from(area.min);
        let far = SectionPos::from(area.max);
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

    pub fn column_count(&self) -> usize {
        (self.dim_x * self.dim_z) as usize
    }

    pub fn column_index(&self, section_index: usize) -> usize {
        section_index % self.column_count()
    }

    pub fn column_step(&self, column_index: usize, dx: i32, dz: i32) -> Option<usize> {
        let x = (column_index as i32 % self.dim_x) + dx;
        let z = (column_index as i32 / self.dim_x) + dz;
        (x >= 0 && x < self.dim_x && z >= 0 && z < self.dim_z)
            .then(|| (z * self.dim_x + x) as usize)
    }

    pub fn cell_count(&self) -> usize {
        self.section_count() * SectionPos::VOLUME
    }

    fn section_stride_z(&self) -> i32 {
        self.dim_x
    }

    fn section_stride_y(&self) -> i32 {
        self.dim_x * self.dim_z
    }

    pub fn sections(&self) -> impl Iterator<Item = (usize, SectionPos)> + '_ {
        (0..self.section_count()).map(move |i| (i, self.section_pos(i)))
    }

    pub fn section_pos(&self, section_index: usize) -> SectionPos {
        let i = section_index as i32;
        let sx = i % self.dim_x;
        let sz = (i / self.dim_x) % self.dim_z;
        let sy = i / (self.dim_x * self.dim_z);
        SectionPos::new(self.origin.x + sx, self.origin.y + sy, self.origin.z + sz)
    }

    pub fn section_box(&self) -> SectionBox {
        SectionBox {
            min: self.origin,
            max: SectionPos::new(
                self.origin.x + self.dim_x - 1,
                self.origin.y + self.dim_y - 1,
                self.origin.z + self.dim_z - 1,
            ),
        }
    }

    pub fn block_bounds(&self) -> BoundingBox {
        let far = SectionPos::new(
            self.origin.x + self.dim_x - 1,
            self.origin.y + self.dim_y - 1,
            self.origin.z + self.dim_z - 1,
        );
        BoundingBox::of_section(self.origin).union(BoundingBox::of_section(far))
    }

    #[inline]
    pub fn section_base(&self, section_index: usize) -> CellIndex {
        (section_index as CellIndex) << LOCAL_BITS
    }

    #[inline]
    pub fn step(&self, index: CellIndex, dir: Direction) -> Option<CellIndex> {
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

        let coord = (index >> LOCAL_BITS) as i32 / section_stride % dim;
        let next = coord + if forward { 1 } else { -1 };
        if next < 0 || next >= dim {
            return None;
        }
        let jump = (section_stride << LOCAL_BITS) - 15 * cell;
        Some((index as i32 + if forward { jump } else { -jump }) as CellIndex)
    }
}

pub struct BlockSnapshot {
    sections: Vec<SectionSource>,
}

pub enum SectionSource {
    Loaded(Arc<SectionBlocks>),
    Open(VoxelId),
    Absent(VoxelId),
}

impl SectionSource {
    #[inline]
    pub fn block_at(&self, local: LocalPos) -> VoxelId {
        match self {
            SectionSource::Loaded(blocks) => blocks.get(local).into(),
            SectionSource::Open(block) | SectionSource::Absent(block) => *block,
        }
    }

    pub fn uniform_block(&self) -> Option<VoxelId> {
        match self {
            SectionSource::Loaded(blocks) => match &blocks.0 {
                PalettedContainer::Homogeneous(block) => Some((*block).into()),
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

    pub fn of(layout: &FieldLayout, blocks: &Snapshot, registry: &LightRegistry) -> Self {
        let sections = layout
            .sections()
            .map(|(_, at)| match blocks.shared(at) {
                Ok(section) => SectionSource::Loaded(Arc::clone(section)),
                Err(Unavailable::Outside) => SectionSource::Open(registry.outside()),
                Err(_) => SectionSource::Absent(registry.unloaded()),
            })
            .collect();
        Self::new(layout, sections)
    }

    pub fn section(&self, section_index: usize) -> Option<&SectionSource> {
        self.sections.get(section_index)
    }

    #[inline]
    pub fn get(&self, index: CellIndex) -> Option<VoxelId> {
        self.sections
            .get((index >> LOCAL_BITS) as usize)
            .map(|section| section.block_at(LocalPos::from_index((index & LOCAL_MASK) as usize)))
    }

    pub fn section_loaded(&self, section_index: usize) -> bool {
        matches!(
            self.sections.get(section_index),
            Some(SectionSource::Loaded(_))
        )
    }

    pub fn section_holds_light(&self, section_index: usize) -> bool {
        matches!(
            self.sections.get(section_index),
            Some(SectionSource::Loaded(_) | SectionSource::Open(_))
        )
    }
}

pub struct LightField {
    layout: FieldLayout,
    cells: Vec<AtomicU8>,
}

impl LightField {
    pub fn new(layout: FieldLayout) -> Self {
        let zeros = vec![0u8; layout.cell_count()];
        let mut zeros = std::mem::ManuallyDrop::new(zeros);
        // SAFETY: `AtomicU8` has the size, alignment and bit validity of `u8`, so the buffer of `zeros` with its
        // length and capacity is a valid buffer of zeroed atomics.
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
    pub fn get(&self, index: CellIndex) -> Option<LightLevel> {
        self.cells
            .get(index as usize)
            .map(|cell| LightLevel::new(cell.load(Ordering::Relaxed)))
    }

    #[inline]
    pub fn set(&self, index: CellIndex, level: LightLevel) {
        if let Some(cell) = self.cells.get(index as usize) {
            cell.store(level.get(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn raise(&self, index: CellIndex, level: LightLevel) -> bool {
        self.cells
            .get(index as usize)
            .is_some_and(|cell| cell.fetch_max(level.get(), Ordering::Relaxed) < level.get())
    }

    pub fn fill_section(&mut self, section_index: usize, light: &LightStorage) {
        if matches!(light, LightStorage::Empty) {
            return;
        }
        let base = section_index * SectionPos::VOLUME;
        let Some(section) = self.cells.get_mut(base..base + SectionPos::VOLUME) else {
            return;
        };
        let mut cells = [0u8; SectionPos::VOLUME];
        light.write_field(&mut cells);
        for (cell, value) in section.iter_mut().zip(cells) {
            *cell.get_mut() = value;
        }
    }

    pub fn section_light_if_changed(
        &self,
        section_index: usize,
        published: Option<&LightStorage>,
    ) -> Option<LightStorage> {
        let cells = self.section_cells(section_index)?;
        match published {
            Some(published) if published.matches_field(&cells) => None,
            _ => Some(LightStorage::from_field(&cells)),
        }
    }

    fn section_cells(&self, section_index: usize) -> Option<[u8; SectionPos::VOLUME]> {
        let base = section_index * SectionPos::VOLUME;
        let section = self.cells.get(base..base + SectionPos::VOLUME)?;
        let mut cells = [0u8; SectionPos::VOLUME];
        for (value, cell) in cells.iter_mut().zip(section) {
            *value = cell.load(Ordering::Relaxed);
        }
        Some(cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> FieldLayout {
        FieldLayout::covering(BoundingBox {
            min: BlockPos::new(0, 0, 0),
            max: BlockPos::new(47, 47, 47),
        })
    }

    #[test]
    fn a_step_moves_one_block_along_its_own_axis_inside_and_across_a_section_seam() {
        let layout = layout();
        let inside = Direction::all().map(|dir| (BlockPos::new(20, 20, 20), dir));
        let across_a_seam = [
            (BlockPos::new(15, 20, 20), Direction::East),
            (BlockPos::new(16, 20, 20), Direction::West),
            (BlockPos::new(20, 15, 20), Direction::Up),
            (BlockPos::new(20, 16, 20), Direction::Down),
            (BlockPos::new(20, 20, 15), Direction::South),
            (BlockPos::new(20, 20, 16), Direction::North),
        ];
        for (start, dir) in inside.into_iter().chain(across_a_seam) {
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
        let section = SectionPos::from(pos);
        let index = (0..layout.section_count())
            .find(|&i| layout.section_pos(i) == section)
            .expect("section is in the area");
        layout.section_base(index) | LocalPos::from(pos).index() as CellIndex
    }

    fn block_of(layout: &FieldLayout, index: CellIndex) -> BlockPos {
        block_in(
            layout.section_pos((index >> LOCAL_BITS) as usize),
            LocalPos::from_index((index & LOCAL_MASK) as usize),
        )
    }
}
