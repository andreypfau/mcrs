use std::sync::Arc;

use rustc_hash::FxHashMap;

use bevy_ecs::prelude::Entity;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_math::{BlockPos, ChunkPos, ColumnPos};
use mcrs_voxel_storage::{ColumnHeights, PalettedContainer, VoxelId};

use crate::SectionBlocks;
use crate::block::{Layer, LightRegistry};
use crate::level::{BlockColumn, LightBounds, LightLevel, LocalPos, SECTION_WIDTH};
use crate::region::BlockBox;
use crate::storage::LightStorage;

/// One loaded section: its blocks, the entity that owns them, and the last
/// published light for both layers.
#[derive(Clone, Debug)]
pub struct Section {
    pub entity: Entity,
    pub blocks: Arc<SectionBlocks>,
    pub block_light: LightStorage,
    pub sky_light: LightStorage,
    /// Whether `entity` has been handed light yet. A dark section's answer is
    /// the same as the nothing it starts with, so without this an epoch could
    /// skip publishing it and the entity would never gain the components a
    /// column is not sent without.
    pub lit: bool,
    /// Whether any block here emits. Read off the palette once, when the section
    /// arrives, because the answer cannot change while the blocks do not: ground
    /// straight out of the generator holds no torch and no redstone, and a field
    /// of such sections has no block layer to compute at all.
    pub emits: bool,
}

impl Section {
    pub fn new(registry: &LightRegistry, entity: Entity, blocks: Arc<SectionBlocks>) -> Self {
        let emits = match &blocks.0 {
            PalettedContainer::Homogeneous(block) => !registry.emission(*block).is_zero(),
            // The palette keeps entries a `set` has emptied, so this errs towards
            // saying yes, which only ever costs the work it would have skipped.
            PalettedContainer::Heterogeneous(data) => data
                .palette
                .iter()
                .any(|block| !registry.emission(*block).is_zero()),
        };
        Self {
            entity,
            blocks,
            block_light: LightStorage::Empty,
            sky_light: LightStorage::Empty,
            lit: false,
            emits,
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
    pub(crate) fn new(default_y: i32) -> Self {
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

    /// Whether every cell from `y` up this column is a sky source.
    pub fn is_source(&self, x: i32, y: i32, z: i32) -> bool {
        y >= self.get(BlockColumn { x, z })
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
        /// Who owns the blocks. Published light is written back to it, so the
        /// engine never has to infer a lifecycle it does not define.
        entity: Entity,
        blocks: Arc<SectionBlocks>,
    },
    UnloadSection {
        pos: ChunkPos,
    },
    /// Hands the sky scan a bound for one column. Carries no blocks, so it
    /// queues no lighting work; any later edit to the column supersedes it.
    SetColumnSurface {
        column: ColumnPos,
        surface: Arc<ColumnSurface>,
    },
}

impl Edit {
    /// The section column whose queued work this edit joins.
    pub fn column(&self) -> ColumnPos {
        match self {
            Edit::SetBlock { pos, .. } => ColumnPos::from(*pos),
            Edit::LoadSection { pos, .. } | Edit::UnloadSection { pos } => ColumnPos::from(*pos),
            Edit::SetColumnSurface { column, .. } => *column,
        }
    }
}

/// One past the topmost non-air Y of each of the 256 block columns, as the
/// world that owns the blocks reports it — the same heightmap that world
/// already keeps, so handing it over costs no repacking.
///
/// Purely a bound on the sky scan, never a source of truth. It is dropped the
/// moment this world's own copy of the column changes, so a scan is never
/// steered by a claim about blocks it has not been handed yet.
pub type ColumnSurface = ColumnHeights;

#[derive(Clone, Copy)]
enum SkyColumnSection<'a> {
    Uniform(VoxelId),
    Blocks(&'a SectionBlocks),
}

pub struct LightWorld {
    registry: Arc<LightRegistry>,
    bounds: LightBounds,
    sky: bool,
    sections: FxHashMap<ChunkPos, Section>,
    loaded_per_column: FxHashMap<ColumnPos, u32>,
    sky_floors: FxHashMap<ColumnPos, Arc<SkyFloor>>,
    surfaces: FxHashMap<ColumnPos, Arc<ColumnSurface>>,
}

impl LightWorld {
    /// Sentinel floor for a column with no data: no cell in it is a source.
    pub const NO_SKY_SOURCES: i32 = i32::MAX;

    pub fn new(registry: Arc<LightRegistry>, bounds: LightBounds) -> Self {
        Self {
            registry,
            bounds,
            sky: true,
            sections: FxHashMap::default(),
            loaded_per_column: FxHashMap::default(),
            sky_floors: FxHashMap::default(),
            surfaces: FxHashMap::default(),
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
        self.forget_column_surface(ColumnPos::from(pos));
        if self.sections.insert(pos, section).is_none() {
            *self.loaded_per_column.entry(ColumnPos::from(pos)).or_default() += 1;
        }
    }

    pub(crate) fn remove_section(&mut self, pos: ChunkPos) -> Option<Section> {
        let section = self.sections.remove(&pos)?;
        let column = ColumnPos::from(pos);
        if let Some(count) = self.loaded_per_column.get_mut(&column) {
            *count -= 1;
            if *count == 0 {
                self.loaded_per_column.remove(&column);
            }
        }
        Some(section)
    }

    pub(crate) fn set_column_surface(&mut self, column: ColumnPos, surface: Arc<ColumnSurface>) {
        self.surfaces.insert(column, surface);
    }

    /// Dropped on every change to the column's blocks: a bound computed against
    /// one set of blocks says nothing about another.
    pub(crate) fn forget_column_surface(&mut self, column: ColumnPos) {
        self.surfaces.remove(&column);
    }

    fn column_surface(&self, column: ColumnPos) -> Option<&ColumnSurface> {
        self.surfaces.get(&column).map(Arc::as_ref)
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
            Some(section) => section.blocks.get(pos),
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

    /// The published floors of one section column, shared rather than copied:
    /// a job reads them from a worker thread while the world keeps scanning.
    pub(crate) fn sky_floors_of(&self, section_column: ColumnPos) -> Option<Arc<SkyFloor>> {
        self.sky_floors.get(&section_column).cloned()
    }

    /// Recomputes the sky source floor for one column by scanning it top down,
    /// and returns the previous floor and the new one.
    ///
    /// Scanning stops at the first occluded seam, so an edit that does not touch
    /// the boundary costs a few reads rather than a full column.
    pub(crate) fn rescan_sky_floor(&mut self, column: BlockColumn) -> (i32, i32) {
        if !self.sky {
            return (Self::NO_SKY_SOURCES, Self::NO_SKY_SOURCES);
        }
        let previous = self.sky_floor(column);
        let section_column = column.section_column();
        let floor = self.scan_sky_floor(
            self.registry.outside(),
            self.sky_column_top_down(section_column),
            local_x_of(column),
            local_z_of(column),
            self.column_surface(section_column),
        );
        let bottom = self.bounds.min_light_y();
        Arc::make_mut(
            self.sky_floors
                .entry(section_column)
                .or_insert_with(|| Arc::new(SkyFloor::new(bottom))),
        )
        .set(column, floor);
        (previous, floor)
    }

    /// Rescans all 256 sky source floors of one section column and reports the
    /// run of cells whose source flag moved, if any.
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
        let (start, entering) = self.skip_uniform_sky(&stack);
        let surfaces = self.column_surface(section_column);
        let published = self.sky_floors.get(&section_column);
        let mut floors = SkyFloor::new(self.bounds.min_light_y());
        let mut moved: Option<(i32, i32)> = None;
        for column in Self::block_columns_of(section_column) {
            let (local_x, local_z) = (local_x_of(column), local_z_of(column));
            let floor = self.scan_sky_floor(
                entering,
                stack[start..].iter().copied(),
                local_x,
                local_z,
                surfaces,
            );
            floors.set(column, floor);
            let previous = published.map_or(Self::NO_SKY_SOURCES, |f| f.get(column));
            if let Some((low, high)) = self.bounds.sky_flip_span(previous, floor) {
                moved = Some(match moved {
                    Some((lo, hi)) => (lo.min(low), hi.max(high)),
                    None => (low, high),
                });
            }
        }
        self.sky_floors.insert(section_column, Arc::new(floors));
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
        entering: VoxelId,
        stack: impl Iterator<Item = (i32, SkyColumnSection<'a>)>,
        local_x: u8,
        local_z: u8,
        surface: Option<&ColumnSurface>,
    ) -> i32 {
        // Above every block, so a column with no bound skips nothing.
        let surface = surface.map_or(i32::MAX, |surface| {
            surface.get(local_x as usize, local_z as usize)
        });
        let mut above = entering;
        for (section_y, section) in stack {
            let top = |block| {
                if self.registry.breaks_sky_column(above, block) {
                    return Err(section_y * SECTION_WIDTH + SECTION_WIDTH);
                }
                if self.registry.breaks_sky_column(block, block) {
                    return Err(section_y * SECTION_WIDTH + SECTION_WIDTH - 1);
                }
                Ok(block)
            };
            match section {
                SkyColumnSection::Uniform(block) => match top(block) {
                    Ok(block) => above = block,
                    Err(floor) => return floor,
                },
                // Every block this column holds here is air, so the section
                // seals no seam a uniform one would not: the entry seam and one
                // air-over-air test settle all sixteen levels.
                SkyColumnSection::Blocks(blocks) if section_y * SECTION_WIDTH >= surface => {
                    let air =
                        blocks.get_cell(local_x as usize, BLOCKS::MASK, local_z as usize);
                    match top(air) {
                        Ok(block) => above = block,
                        Err(floor) => return floor,
                    }
                }
                SkyColumnSection::Blocks(blocks) => {
                    for local_y in (0..BLOCKS::SIZE as u8).rev() {
                        let below =
                            blocks.get_cell(local_x as usize, local_y as usize, local_z as usize);
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

    /// How much of a column stack every one of its 256 block columns answers
    /// the same way, and the block the scan then enters on.
    ///
    /// A uniform section that seals no seam is transparent to all 256 columns
    /// alike, so the empty sky above the terrain is walked once per stack
    /// instead of once per column.
    fn skip_uniform_sky(&self, stack: &[(i32, SkyColumnSection<'_>)]) -> (usize, VoxelId) {
        let mut above = self.registry.outside();
        let mut start = 0;
        for (index, (_, section)) in stack.iter().enumerate() {
            let SkyColumnSection::Uniform(block) = section else {
                break;
            };
            if self.registry.breaks_sky_column(above, *block)
                || self.registry.breaks_sky_column(*block, *block)
            {
                break;
            }
            above = *block;
            start = index + 1;
        }
        (start, above)
    }

    pub(crate) fn forget_sky_floors(&mut self, section_column: ColumnPos) {
        self.sky_floors.remove(&section_column);
    }

    /// True when the section column still holds at least one loaded section.
    pub(crate) fn column_is_loaded(&self, section_column: ColumnPos) -> bool {
        self.loaded_per_column.contains_key(&section_column)
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
