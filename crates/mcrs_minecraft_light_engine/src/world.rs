use std::sync::Arc;

use rustc_hash::FxHashMap;

use mcrs_minecraft_chunk::{ColumnHeights, PalettedContainer, VoxelId};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

use crate::SectionBlocks;
use crate::block::LightRegistry;
use crate::level::{BlockColumn, LightBounds, LightLevel, SECTION_WIDTH};
use mcrs_minecraft_chunk::LightLayer;
use mcrs_minecraft_chunk::LightStorage;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_volume::{
    Access, BlockVolume, ColumnLayout, PalettedSection, SectionBox, Snapshot, Unavailable, Volume,
};
use mcrs_voxel::LocalPos;

#[derive(Clone, Debug)]
pub struct Section {
    pub entity: u64,
    pub blocks: Arc<SectionBlocks>,
    pub block_light: LightStorage,
    pub sky_light: LightStorage,
    pub lit: bool,
    pub emits: bool,
}

impl Section {
    pub fn new(registry: &LightRegistry, entity: u64, blocks: Arc<SectionBlocks>) -> Self {
        let emits = match &blocks.0 {
            PalettedContainer::Homogeneous(block) => !registry.emission((*block).into()).is_zero(),
            PalettedContainer::Heterogeneous(data) => data
                .palette
                .iter()
                .any(|block| !registry.emission((*block).into()).is_zero()),
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

    pub fn light(&self, layer: LightLayer) -> &LightStorage {
        match layer {
            LightLayer::Block => &self.block_light,
            LightLayer::Sky => &self.sky_light,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkyFloor {
    lowest_source_y: Box<[i32; SectionPos::AREA]>,
}

impl SkyFloor {
    pub(crate) fn new(default_y: i32) -> Self {
        Self {
            lowest_source_y: Box::new([default_y; SectionPos::AREA]),
        }
    }

    fn index(column: BlockColumn) -> usize {
        ((column.x as usize) & SectionPos::MASK)
            | (((column.z as usize) & SectionPos::MASK) << SectionPos::BITS)
    }

    pub fn get(&self, column: BlockColumn) -> i32 {
        self.lowest_source_y
            .get(Self::index(column))
            .copied()
            .unwrap_or(LightWorld::NO_SKY_SOURCES)
    }

    pub fn is_source(&self, x: i32, y: i32, z: i32) -> bool {
        y >= self.get(BlockColumn { x, z })
    }

    fn set(&mut self, column: BlockColumn, y: i32) {
        if let Some(floor) = self.lowest_source_y.get_mut(Self::index(column)) {
            *floor = y;
        }
    }
}

#[derive(Clone, Debug)]
pub enum Edit {
    SetBlock {
        pos: BlockPos,
        block: BlockStateId,
    },
    LoadSection {
        pos: SectionPos,
        entity: u64,
        blocks: Arc<SectionBlocks>,
    },
    UnloadSection {
        pos: SectionPos,
    },
    SetColumnSurface {
        column: ColumnPos,
        surface: Arc<ColumnSurface>,
    },
}

impl Edit {
    pub fn column(&self) -> ColumnPos {
        match self {
            Edit::SetBlock { pos, .. } => ColumnPos::from(*pos),
            Edit::LoadSection { pos, .. } | Edit::UnloadSection { pos } => ColumnPos::from(*pos),
            Edit::SetColumnSurface { column, .. } => *column,
        }
    }
}

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
    sections: FxHashMap<SectionPos, Section>,
    loaded_per_column: FxHashMap<ColumnPos, u32>,
    sky_floors: FxHashMap<ColumnPos, Arc<SkyFloor>>,
    surfaces: FxHashMap<ColumnPos, Arc<ColumnSurface>>,
}

impl LightWorld {
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

    pub fn section(&self, pos: SectionPos) -> Option<&Section> {
        self.sections.get(&pos)
    }

    pub fn section_mut(&mut self, pos: SectionPos) -> Option<&mut Section> {
        self.sections.get_mut(&pos)
    }

    pub fn loaded_sections(&self) -> impl Iterator<Item = (&SectionPos, &Section)> {
        self.sections.iter()
    }

    pub(crate) fn insert_section(&mut self, pos: SectionPos, section: Section) {
        self.forget_column_surface(ColumnPos::from(pos));
        if self.sections.insert(pos, section).is_none() {
            *self
                .loaded_per_column
                .entry(ColumnPos::from(pos))
                .or_default() += 1;
        }
    }

    pub(crate) fn remove_section(&mut self, pos: SectionPos) -> Option<Section> {
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

    pub(crate) fn forget_column_surface(&mut self, column: ColumnPos) {
        self.surfaces.remove(&column);
    }

    fn column_surface(&self, column: ColumnPos) -> Option<&ColumnSurface> {
        self.surfaces.get(&column).map(Arc::as_ref)
    }

    pub fn block_at(&self, pos: BlockPos) -> VoxelId {
        if self.bounds.is_outside(pos.y) {
            return self.registry.outside();
        }
        match self.sections.get(&SectionPos::from(pos)) {
            Some(section) => section.blocks.get(LocalPos::from(pos)).into(),
            None => self.registry.unloaded(),
        }
    }

    pub(crate) fn is_outside_world(&self, pos: SectionPos) -> bool {
        self.bounds.is_outside(pos.y * SECTION_WIDTH)
    }

    pub fn light_at(&self, pos: BlockPos, layer: LightLayer) -> LightLevel {
        match self.sections.get(&SectionPos::from(pos)) {
            Some(section) => {
                let local = LocalPos::from(pos);
                LightLevel::new(section.light(layer).get(
                    local.x() as usize,
                    local.y() as usize,
                    local.z() as usize,
                ))
            }
            None => match layer {
                LightLayer::Block => LightLevel::ZERO,
                LightLayer::Sky => LightLevel::MAX,
            },
        }
    }

    pub fn sky_floor(&self, column: BlockColumn) -> i32 {
        match self.sky_floors.get(&column.section_column()) {
            Some(floor) => floor.get(column),
            None => Self::NO_SKY_SOURCES,
        }
    }

    pub(crate) fn sky_floors_of(&self, section_column: ColumnPos) -> Option<Arc<SkyFloor>> {
        self.sky_floors.get(&section_column).cloned()
    }

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

    pub(crate) fn rescan_column(&mut self, section_column: ColumnPos) -> Option<BoundingBox> {
        if !self.sky {
            return None;
        }
        let stack: Vec<_> = self.sky_column_top_down(section_column).collect();
        let (below_uniform, entering) = self.skip_uniform_sky(&stack);
        let surfaces = self.column_surface(section_column);
        let published = self.sky_floors.get(&section_column);
        let mut floors = SkyFloor::new(self.bounds.min_light_y());
        let mut moved: Option<(i32, i32)> = None;
        for column in Self::block_columns_of(section_column) {
            let (local_x, local_z) = (local_x_of(column), local_z_of(column));
            let floor = self.scan_sky_floor(
                entering,
                below_uniform.iter().copied(),
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
            BoundingBox {
                min: BlockPos::new(x, low, z),
                max: BlockPos::new(x + SECTION_WIDTH - 1, high, z + SECTION_WIDTH - 1),
            }
        })
    }

    fn sky_column_top_down(
        &self,
        section_column: ColumnPos,
    ) -> impl Iterator<Item = (i32, SkyColumnSection<'_>)> {
        self.bounds.light_sections().rev().map(move |section_y| {
            let pos = SectionPos::new(section_column.x, section_y, section_column.z);
            let section = match self.sections.get(&pos) {
                Some(section) => match &section.blocks.0 {
                    PalettedContainer::Homogeneous(block) => {
                        SkyColumnSection::Uniform((*block).into())
                    }
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

    fn scan_sky_floor<'a>(
        &self,
        entering: VoxelId,
        stack: impl Iterator<Item = (i32, SkyColumnSection<'a>)>,
        local_x: u8,
        local_z: u8,
        surface: Option<&ColumnSurface>,
    ) -> i32 {
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
                SkyColumnSection::Blocks(blocks) if section_y * SECTION_WIDTH >= surface => {
                    let air = blocks
                        .get(LocalPos::new(local_x, SectionPos::MASK as u8, local_z))
                        .into();
                    match top(air) {
                        Ok(block) => above = block,
                        Err(floor) => return floor,
                    }
                }
                SkyColumnSection::Blocks(blocks) => {
                    for local_y in (0..SectionPos::SIZE as u8).rev() {
                        let below = blocks.get(LocalPos::new(local_x, local_y, local_z)).into();
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

    fn skip_uniform_sky<'s, 'a>(
        &self,
        mut stack: &'s [(i32, SkyColumnSection<'a>)],
    ) -> (&'s [(i32, SkyColumnSection<'a>)], VoxelId) {
        let mut above = self.registry.outside();
        while let Some(((_, SkyColumnSection::Uniform(block)), below)) = stack.split_first() {
            if self.registry.breaks_sky_column(above, *block)
                || self.registry.breaks_sky_column(*block, *block)
            {
                break;
            }
            above = *block;
            stack = below;
        }
        (stack, above)
    }

    pub(crate) fn forget_sky_floors(&mut self, section_column: ColumnPos) {
        self.sky_floors.remove(&section_column);
    }

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

impl Volume for LightWorld {
    fn bounds(&self) -> BoundingBox {
        BoundingBox {
            min: BlockPos::new(i32::MIN, self.bounds.min_block_y(), i32::MIN),
            max: BlockPos::new(i32::MAX, self.bounds.max_block_y(), i32::MAX),
        }
    }

    fn access(&self, section: SectionPos) -> Access {
        if self.is_outside_world(section) {
            Access::Outside
        } else if self.sections.contains_key(&section) {
            Access::Read
        } else {
            Access::Unloaded
        }
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        Some(ColumnLayout {
            min_section_y: self.bounds.min_section_y,
            sections: self
                .bounds
                .max_section_y
                .abs_diff(self.bounds.min_section_y)
                .saturating_add(1),
        })
    }
}

impl LightWorld {
    fn shared(&self, at: SectionPos) -> Result<&Arc<SectionBlocks>, Unavailable> {
        match self.sections.get(&at) {
            Some(section) => Ok(&section.blocks),
            None if self.is_outside_world(at) => Err(Unavailable::Outside),
            None => Err(Unavailable::Unloaded),
        }
    }
}

impl BlockVolume for LightWorld {
    type Section<'a> = PalettedSection<'a>;

    fn section(&self, at: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        self.shared(at).map(|blocks| PalettedSection(&blocks.0))
    }
}

impl LightWorld {
    pub fn snapshot(&self, area: SectionBox) -> Snapshot {
        Snapshot::take(area, self.column_layout(), |at| self.shared(at).cloned())
    }
}

fn local_x_of(column: BlockColumn) -> u8 {
    (column.x & SectionPos::MASK as i32) as u8
}

fn local_z_of(column: BlockColumn) -> u8 {
    (column.z & SectionPos::MASK as i32) as u8
}
