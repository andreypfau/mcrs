use std::collections::HashMap;
use std::sync::Arc;

use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_math::{BlockPos, ChunkPos, ColumnPos};
use mcrs_voxel_storage::{PalettedContainer, VoxelId};

use crate::block::{Layer, LightRegistry};
use crate::level::{BlockColumn, LightBounds, LightLevel, LocalPos, SECTION_WIDTH};
use crate::region::BlockBox;
use crate::section::{self, SectionBlocks};
use crate::storage::LightStorage;

/// One loaded section: its blocks, plus the last published light for both layers.
#[derive(Clone, Debug)]
pub struct Section {
    pub blocks: Arc<SectionBlocks>,
    pub block_light: LightStorage,
    pub sky_light: LightStorage,
}

impl Section {
    pub fn new(blocks: Arc<SectionBlocks>) -> Self {
        Self {
            blocks,
            block_light: LightStorage::Empty,
            sky_light: LightStorage::Empty,
        }
    }

    pub fn light(&self, layer: Layer) -> &LightStorage {
        match layer {
            Layer::Block => &self.block_light,
            Layer::Sky => &self.sky_light,
        }
    }
}

/// Lowest block Y that is still a sky source, for each of the 256 block columns
/// of one section column.
///
/// This is the whole of the sky source rule: `E_sky(p) == 15` exactly when
/// `p.y >= floor(p.column())`. It is cached rather than recomputed because
/// deciding which cells an edit can affect needs the value from *before* the
/// edit, and by the time the calculation runs the blocks have already changed.
#[derive(Clone, Debug)]
pub struct SkyFloor {
    lowest_source_y: Box<[i32]>,
}

impl SkyFloor {
    fn new(default_y: i32) -> Self {
        Self {
            lowest_source_y: vec![default_y; BLOCKS::AREA].into_boxed_slice(),
        }
    }

    fn index(column: BlockColumn) -> usize {
        ((column.x as usize) & BLOCKS::MASK)
            | (((column.z as usize) & BLOCKS::MASK) << BLOCKS::BITS)
    }

    pub fn get(&self, column: BlockColumn) -> i32 {
        self.lowest_source_y[Self::index(column)]
    }

    fn set(&mut self, column: BlockColumn, y: i32) {
        self.lowest_source_y[Self::index(column)] = y;
    }
}

/// A change the lighting subsystem must react to.
///
/// Loading and unloading are edits like any other: a section that just appeared
/// has to enter the same path, or it is simply never lit.
#[derive(Clone, Debug)]
pub enum Edit {
    SetBlock {
        pos: BlockPos,
        block: VoxelId,
    },
    LoadSection {
        pos: ChunkPos,
        blocks: Arc<SectionBlocks>,
    },
    UnloadSection {
        pos: ChunkPos,
    },
}

impl Edit {
    /// The section column whose queued work this edit joins.
    pub fn column(&self) -> ColumnPos {
        match self {
            Edit::SetBlock { pos, .. } => ColumnPos::from(*pos),
            Edit::LoadSection { pos, .. } | Edit::UnloadSection { pos } => ColumnPos::from(*pos),
        }
    }
}

#[derive(Clone, Copy)]
enum SkyColumnSection<'a> {
    Uniform(VoxelId),
    Blocks(&'a SectionBlocks),
}

pub struct LightWorld {
    registry: Arc<LightRegistry>,
    bounds: LightBounds,
    sky: bool,
    sections: HashMap<ChunkPos, Section>,
    sky_floors: HashMap<ColumnPos, SkyFloor>,
}

impl LightWorld {
    /// Sentinel floor for a column with no data: no cell in it is a source.
    pub const NO_SKY_SOURCES: i32 = i32::MAX;

    pub fn new(registry: Arc<LightRegistry>, bounds: LightBounds) -> Self {
        Self {
            registry,
            bounds,
            sky: true,
            sections: HashMap::new(),
            sky_floors: HashMap::new(),
        }
    }

    /// For a dimension whose ceiling is not the sky: no cell anywhere is a sky
    /// source, so the sky layer stays at zero however the world is shaped.
    pub fn without_sky(mut self) -> Self {
        self.sky = false;
        self
    }

    pub fn registry(&self) -> &Arc<LightRegistry> {
        &self.registry
    }

    pub fn bounds(&self) -> LightBounds {
        self.bounds
    }

    pub fn section(&self, pos: ChunkPos) -> Option<&Section> {
        self.sections.get(&pos)
    }

    pub fn section_mut(&mut self, pos: ChunkPos) -> Option<&mut Section> {
        self.sections.get_mut(&pos)
    }

    pub fn loaded_sections(&self) -> impl Iterator<Item = (&ChunkPos, &Section)> {
        self.sections.iter()
    }

    pub(crate) fn insert_section(&mut self, pos: ChunkPos, section: Section) {
        self.sections.insert(pos, section);
    }

    pub(crate) fn remove_section(&mut self, pos: ChunkPos) -> Option<Section> {
        self.sections.remove(&pos)
    }

    /// The block at a position.
    ///
    /// Above and below the world there are no blocks, so light passes freely —
    /// that is where sky light comes from. Inside the world but not loaded,
    /// light stops, so that an edit near the loading frontier does not flood
    /// the dark beyond it.
    pub fn block_at(&self, pos: BlockPos) -> VoxelId {
        if self.bounds.is_outside(pos.y) {
            return self.registry.outside();
        }
        match self.sections.get(&ChunkPos::from(pos)) {
            Some(section) => section::block_at(&section.blocks, local_of(pos)),
            None => self.registry.unloaded(),
        }
    }

    /// Whether a missing section sits outside the world rather than merely
    /// being unloaded.
    pub(crate) fn is_outside_world(&self, pos: ChunkPos) -> bool {
        self.bounds.is_outside(pos.y * SECTION_WIDTH)
    }

    /// Light as seen from outside the engine.
    ///
    /// Where no data exists the reader sees 0 for block light and 15 for sky
    /// light. The calculation itself uses a different convention — unloaded
    /// space contributes nothing to either layer — which is why this method is
    /// not used to build seeds.
    pub fn light_at(&self, pos: BlockPos, layer: Layer) -> LightLevel {
        match self.sections.get(&ChunkPos::from(pos)) {
            Some(section) => {
                let local = local_of(pos);
                LightLevel::new(section.light(layer).get(
                    local.x() as usize,
                    local.y() as usize,
                    local.z() as usize,
                ))
            }
            None => match layer {
                Layer::Block => LightLevel::ZERO,
                Layer::Sky => LightLevel::MAX,
            },
        }
    }

    /// Combined brightness at a position, as [`LightLevel::brightness`] defines it.
    pub fn brightness(&self, pos: BlockPos, sky_darken: u8) -> LightLevel {
        LightLevel::brightness(
            self.light_at(pos, Layer::Block),
            self.light_at(pos, Layer::Sky),
            sky_darken,
        )
    }

    /// Lowest block Y in this column that is still a sky source, or
    /// [`Self::NO_SKY_SOURCES`] where the world knows nothing about the column.
    ///
    /// An unloaded column contributes nothing rather than counting as open sky.
    /// Treating it as open would light the padding sections beside a sealed
    /// world, and that light would then leak inward: entering a block costs
    /// something, but leaving one is free.
    pub fn sky_floor(&self, column: BlockColumn) -> i32 {
        match self.sky_floors.get(&column.section_column()) {
            Some(floor) => floor.get(column),
            None => Self::NO_SKY_SOURCES,
        }
    }

    /// Recomputes the sky source floor for one column by scanning it top down,
    /// and returns the previous value.
    ///
    /// Scanning stops at the first occluded seam, so an edit that does not touch
    /// the boundary costs a few reads rather than a full column.
    pub(crate) fn rescan_sky_floor(&mut self, column: BlockColumn) -> i32 {
        if !self.sky {
            return Self::NO_SKY_SOURCES;
        }
        let previous = self.sky_floor(column);
        let section_column = column.section_column();
        let floor = self.scan_sky_floor(
            self.sky_column_top_down(section_column),
            local_x_of(column),
            local_z_of(column),
        );
        let bottom = self.bounds.min_light_y();
        self.sky_floors
            .entry(section_column)
            .or_insert_with(|| SkyFloor::new(bottom))
            .set(column, floor);
        previous
    }

    /// Recomputes all 256 sky source floors of one section column.
    /// Rescans the column's sky floors and reports the run of cells whose
    /// source flag moved, if any.
    ///
    /// A section arriving can move the floor of a column whose lower sections
    /// were lit ticks ago, and those cells are further than one section from
    /// the section that moved it, so the loading influence alone does not reach
    /// them.
    pub(crate) fn rescan_column(&mut self, section_column: ColumnPos) -> Option<BlockBox> {
        if !self.sky {
            return None;
        }
        let stack: Vec<_> = self.sky_column_top_down(section_column).collect();
        let mut floors = SkyFloor::new(self.bounds.min_light_y());
        let mut moved: Option<(i32, i32)> = None;
        for column in Self::block_columns_of(section_column) {
            let floor = self.scan_sky_floor(
                stack.iter().copied(),
                local_x_of(column),
                local_z_of(column),
            );
            floors.set(column, floor);
            let previous = self.sky_floor(column);
            if previous != floor {
                let low = previous.min(floor).max(self.bounds.min_light_y());
                let high = previous
                    .max(floor)
                    .saturating_sub(1)
                    .min(self.bounds.max_light_y());
                if low <= high {
                    moved = Some(match moved {
                        Some((lo, hi)) => (lo.min(low), hi.max(high)),
                        None => (low, high),
                    });
                }
            }
        }
        self.sky_floors.insert(section_column, floors);
        moved.map(|(low, high)| {
            let (x, z) = (
                section_column.x * SECTION_WIDTH,
                section_column.z * SECTION_WIDTH,
            );
            BlockBox {
                min: BlockPos::new(x, low, z),
                max: BlockPos::new(x + SECTION_WIDTH - 1, high, z + SECTION_WIDTH - 1),
            }
        })
    }

    /// The vertical stack a sky scan reads, top down and resolved only as far
    /// as the scan gets, so an edit that stops at the first seam never touches
    /// the sections below it.
    fn sky_column_top_down(
        &self,
        section_column: ColumnPos,
    ) -> impl Iterator<Item = (i32, SkyColumnSection<'_>)> {
        self.bounds.light_sections().rev().map(move |section_y| {
            let pos = ChunkPos::new(section_column.x, section_y, section_column.z);
            let section = match self.sections.get(&pos) {
                Some(section) => match &section.blocks.0 {
                    PalettedContainer::Homogeneous(block) => SkyColumnSection::Uniform(*block),
                    PalettedContainer::Heterogeneous(_) => {
                        SkyColumnSection::Blocks(&section.blocks)
                    }
                },
                None if self.is_outside_world(pos) => {
                    SkyColumnSection::Uniform(self.registry.outside())
                }
                None => SkyColumnSection::Uniform(self.registry.unloaded()),
            };
            (section_y, section)
        })
    }

    /// Walked section by section: one lookup per sixteen blocks rather than one
    /// per block, which is the difference between this being free and this
    /// dominating a bulk load. A section of one block answers in two seam tests
    /// however tall the empty air above the terrain is — the block entering from
    /// above against the first block, then that block against itself.
    fn scan_sky_floor<'a>(
        &self,
        stack: impl Iterator<Item = (i32, SkyColumnSection<'a>)>,
        local_x: u8,
        local_z: u8,
    ) -> i32 {
        let mut above = self.registry.outside();
        for (section_y, section) in stack {
            match section {
                SkyColumnSection::Uniform(block) => {
                    if self.registry.breaks_sky_column(above, block) {
                        return section_y * SECTION_WIDTH + SECTION_WIDTH;
                    }
                    if self.registry.breaks_sky_column(block, block) {
                        return section_y * SECTION_WIDTH + SECTION_WIDTH - 1;
                    }
                    above = block;
                }
                SkyColumnSection::Blocks(blocks) => {
                    for local_y in (0..BLOCKS::SIZE as u8).rev() {
                        let below =
                            section::block_at(blocks, LocalPos::new(local_x, local_y, local_z));
                        if self.registry.breaks_sky_column(above, below) {
                            return section_y * SECTION_WIDTH + local_y as i32 + 1;
                        }
                        above = below;
                    }
                }
            }
        }
        self.bounds.min_light_y()
    }

    pub(crate) fn forget_sky_floors(&mut self, section_column: ColumnPos) {
        self.sky_floors.remove(&section_column);
    }

    /// True when the section column still holds at least one loaded section.
    pub(crate) fn column_is_loaded(&self, section_column: ColumnPos) -> bool {
        self.bounds.light_sections().any(|y| {
            self.sections
                .contains_key(&ChunkPos::new(section_column.x, y, section_column.z))
        })
    }

    pub(crate) fn block_columns_of(section_column: ColumnPos) -> impl Iterator<Item = BlockColumn> {
        let base_x = section_column.x * SECTION_WIDTH;
        let base_z = section_column.z * SECTION_WIDTH;
        (0..SECTION_WIDTH).flat_map(move |dz| {
            (0..SECTION_WIDTH).map(move |dx| BlockColumn {
                x: base_x + dx,
                z: base_z + dz,
            })
        })
    }
}

fn local_x_of(column: BlockColumn) -> u8 {
    (column.x & BLOCKS::MASK as i32) as u8
}

fn local_z_of(column: BlockColumn) -> u8 {
    (column.z & BLOCKS::MASK as i32) as u8
}

pub(crate) fn local_of(pos: BlockPos) -> LocalPos {
    LocalPos::new(
        (pos.x & BLOCKS::MASK as i32) as u8,
        (pos.y & BLOCKS::MASK as i32) as u8,
        (pos.z & BLOCKS::MASK as i32) as u8,
    )
}
