use crate::ColumnBlocks;
use bevy_ecs::prelude::*;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_block::Block;
use mcrs_minecraft_block::definition::{BlockStateFlags, Blocks};
use mcrs_minecraft_block::tags::{
    BLOCKS_MOTION_IN_HEIGHTMAP, BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES,
};
use mcrs_minecraft_chunk::{ColumnHeights, PalettedContainer, VoxelId};
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_registry::BlockStateId;
use std::cell::Cell;
use std::sync::Arc;

/// Topmost non-air block. The upper bound of every other map.
#[derive(Component, Debug, Clone)]
pub struct SurfaceHeightmap(pub ColumnHeights);

/// Topmost block that blocks motion, leaves included, fluids excluded.
#[derive(Component, Debug, Clone)]
pub struct SolidHeightmap(pub ColumnHeights);

/// Topmost block that blocks motion or holds a fluid.
#[derive(Component, Debug, Clone)]
pub struct MotionHeightmap(pub ColumnHeights);

/// Topmost block that blocks motion without being leaves, or holds a fluid.
#[derive(Component, Debug, Clone)]
pub struct NoLeavesHeightmap(pub ColumnHeights);

bitflags::bitflags! {
    /// The four heightmap predicates, as one bitmask per block state.
    ///
    /// `SOLID` implies `MOTION` and `NO_LEAVES` implies `MOTION` inside a single
    /// mask, so a descent that closes one closes the other in the same step.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct HeightmapKinds: u8 {
        const SURFACE = 1 << 0;
        const SOLID = 1 << 1;
        const MOTION = 1 << 2;
        const NO_LEAVES = 1 << 3;
    }
}

/// Which of the four predicates each block state satisfies, indexed by
/// [`VoxelId`].
#[derive(Resource, Clone, Debug)]
pub struct HeightmapPredicates(Arc<[HeightmapKinds]>);

impl HeightmapPredicates {
    #[inline]
    pub fn get(&self, id: VoxelId) -> HeightmapKinds {
        self.0
            .get(id.0 as usize)
            .copied()
            .unwrap_or(HeightmapKinds::empty())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub fn heightmap_predicates(blocks: &Blocks, tags: &DynTagRegistry<Block>) -> HeightmapPredicates {
    let mut table = Vec::with_capacity(blocks.state_count());
    for index in 0..blocks.state_count() {
        let id = BlockStateId(index as u16);
        let state = blocks.state(id);
        let block = blocks.block_index(id);
        let mut kinds = HeightmapKinds::empty();
        if !state.flags.contains(BlockStateFlags::IS_AIR) {
            kinds |= HeightmapKinds::SURFACE;
        }
        if tags.contains(&BLOCKS_MOTION_IN_HEIGHTMAP, block) {
            kinds |= HeightmapKinds::SOLID | HeightmapKinds::MOTION;
        }
        if tags.contains(&BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES, block) {
            kinds |= HeightmapKinds::NO_LEAVES;
        }
        if state.fluid.is_some() {
            kinds |= HeightmapKinds::MOTION | HeightmapKinds::NO_LEAVES;
        }
        table.push(kinds);
    }
    HeightmapPredicates(table.into())
}

/// The four maps of one column, built off-thread before the column entity
/// exists and inserted on it whole.
#[derive(Bundle, Debug, Clone)]
pub struct ColumnHeightmapSet {
    pub surface: SurfaceHeightmap,
    pub solid: SolidHeightmap,
    pub motion: MotionHeightmap,
    pub no_leaves: NoLeavesHeightmap,
}

impl ColumnHeightmapSet {
    pub fn new(height: u32, min_y: i32) -> Self {
        Self {
            surface: SurfaceHeightmap(ColumnHeights::new(height, min_y)),
            solid: SolidHeightmap(ColumnHeights::new(height, min_y)),
            motion: MotionHeightmap(ColumnHeights::new(height, min_y)),
            no_leaves: NoLeavesHeightmap(ColumnHeights::new(height, min_y)),
        }
    }

    pub fn set(&mut self, kinds: HeightmapKinds, x: usize, z: usize, y: i32) {
        if kinds.contains(HeightmapKinds::SURFACE) {
            self.surface.0.set(x, z, y);
        }
        if kinds.contains(HeightmapKinds::SOLID) {
            self.solid.0.set(x, z, y);
        }
        if kinds.contains(HeightmapKinds::MOTION) {
            self.motion.0.set(x, z, y);
        }
        if kinds.contains(HeightmapKinds::NO_LEAVES) {
            self.no_leaves.0.set(x, z, y);
        }
    }

    pub fn apply_write(
        &mut self,
        x: usize,
        z: usize,
        y: i32,
        kinds: HeightmapKinds,
        predicates: &HeightmapPredicates,
        read: &impl Fn(i32) -> VoxelId,
    ) {
        apply_write(
            &mut self.surface.0,
            &mut self.solid.0,
            &mut self.motion.0,
            &mut self.no_leaves.0,
            x,
            z,
            y,
            kinds,
            predicates,
            read,
        );
    }
}

/// The two maps a placement modifier reads as `WORLD_SURFACE_WG` and
/// `OCEAN_FLOOR_WG`: the column as the fill, the surface rules and the carvers
/// left it, before any feature wrote into it.
#[derive(Debug, Clone)]
pub struct TerrainHeightmaps {
    pub surface: ColumnHeights,
    pub solid: ColumnHeights,
}

/// Carry one block write through the four maps.
///
/// The order is mandatory: `MOTION`'s descent stops at the topmost `SOLID`
/// block, which satisfies `MOTION` too, so `SOLID` must already carry this
/// write before `MOTION` is asked.
#[allow(clippy::too_many_arguments)]
pub fn apply_write(
    surface: &mut ColumnHeights,
    solid: &mut ColumnHeights,
    motion: &mut ColumnHeights,
    no_leaves: &mut ColumnHeights,
    x: usize,
    z: usize,
    y: i32,
    kinds: HeightmapKinds,
    predicates: &HeightmapPredicates,
    read: &impl Fn(i32) -> VoxelId,
) {
    let floor = surface.min_y();
    for (map, kind) in [
        (&mut *surface, HeightmapKinds::SURFACE),
        (&mut *solid, HeightmapKinds::SOLID),
        (&mut *no_leaves, HeightmapKinds::NO_LEAVES),
    ] {
        apply_edit(map, kind, x, z, y, kinds, floor, predicates, read);
    }
    let solid_floor = solid.get(x, z);
    apply_edit(
        motion,
        HeightmapKinds::MOTION,
        x,
        z,
        y,
        kinds,
        solid_floor,
        predicates,
        read,
    );
}

/// The terrain descent, over the dense buffer the fill still holds: the two
/// maps a placement modifier reads as `WORLD_SURFACE_WG` and `OCEAN_FLOOR_WG`.
pub fn build_terrain_heightmaps(
    column: &ColumnBlocks,
    predicates: &HeightmapPredicates,
) -> Option<TerrainHeightmaps> {
    let set = descend(
        column.y_sections(),
        HeightmapKinds::SURFACE.union(HeightmapKinds::SOLID),
        |index| Some(SectionCells::Dense(column.section_cells(index))),
        predicates,
    )?;
    Some(TerrainHeightmaps {
        surface: set.surface.0,
        solid: set.solid.0,
    })
}

/// One descending pass over the column that closes every map at the first block
/// satisfying it.
pub fn build_column_heightmaps(
    sections: &[Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    predicates: &HeightmapPredicates,
) -> Option<ColumnHeightmapSet> {
    descend(
        y_sections,
        HeightmapKinds::all(),
        |index| {
            let (blocks, _) = sections.get(index)?.as_ref()?;
            Some(SectionCells::Palette(&blocks.0))
        },
        predicates,
    )
}

/// A section's blocks as the descent reads them: the fill's dense buffer, or a
/// packed palette, which settles all 256 columns from one lookup when it is
/// homogeneous, so the empty air above the terrain and the open water of an
/// ocean each cost one test rather than one per block.
enum SectionCells<'a> {
    Dense(&'a [Cell<VoxelId>]),
    Palette(&'a PalettedContainer<VoxelId, { SectionPos::SIZE }>),
}

impl SectionCells<'_> {
    fn homogeneous(&self) -> Option<VoxelId> {
        match self {
            SectionCells::Palette(PalettedContainer::Homogeneous(id)) => Some(*id),
            _ => None,
        }
    }

    #[inline]
    fn get(&self, local_y: usize, cell: usize) -> VoxelId {
        match self {
            SectionCells::Dense(cells) => cells[local_y * SectionPos::AREA + cell].get(),
            SectionCells::Palette(palette) => {
                palette.get(cell & SectionPos::MASK, local_y, cell >> SectionPos::BITS)
            }
        }
    }
}

fn descend<'a>(
    y_sections: &[i32],
    wanted: HeightmapKinds,
    section: impl Fn(usize) -> Option<SectionCells<'a>>,
    predicates: &HeightmapPredicates,
) -> Option<ColumnHeightmapSet> {
    let first = *y_sections.first()?;
    let last = *y_sections.last()?;
    let min_y = first * SectionPos::SIZE as i32;
    let height = ((last - first + 1) * SectionPos::SIZE as i32) as u32;

    let mut set = ColumnHeightmapSet::new(height, min_y);
    let mut open = [wanted; SectionPos::AREA];
    let mut remaining = SectionPos::AREA;

    for (index, &section_y) in y_sections.iter().enumerate().rev() {
        if remaining == 0 {
            break;
        }
        let Some(cells) = section(index) else {
            continue;
        };
        let section_min_y = section_y * SectionPos::SIZE as i32;
        if let Some(id) = cells.homogeneous() {
            let kinds = predicates.get(id);
            if kinds.is_empty() {
                continue;
            }
            let top = section_min_y + SectionPos::SIZE as i32;
            for (cell, open) in open.iter_mut().enumerate() {
                remaining -= close(&mut set, open, cell, kinds, top) as usize;
            }
            continue;
        }
        for local_y in (0..SectionPos::SIZE).rev() {
            if remaining == 0 {
                break;
            }
            let top = section_min_y + local_y as i32 + 1;
            for (cell, open) in open.iter_mut().enumerate() {
                if open.is_empty() {
                    continue;
                }
                let kinds = predicates.get(cells.get(local_y, cell));
                remaining -= close(&mut set, open, cell, kinds, top) as usize;
            }
        }
    }
    Some(set)
}

/// Returns whether this block closed the column's last open map.
#[inline]
fn close(
    set: &mut ColumnHeightmapSet,
    open: &mut HeightmapKinds,
    cell: usize,
    kinds: HeightmapKinds,
    top: i32,
) -> bool {
    let newly = *open & kinds;
    if newly.is_empty() {
        return false;
    }
    set.set(
        newly,
        cell & SectionPos::MASK,
        cell >> SectionPos::BITS,
        top,
    );
    open.remove(newly);
    open.is_empty()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_edit(
    map: &mut ColumnHeights,
    kind: HeightmapKinds,
    x: usize,
    z: usize,
    y: i32,
    kinds: HeightmapKinds,
    floor: i32,
    predicates: &HeightmapPredicates,
    read: &impl Fn(i32) -> VoxelId,
) {
    let height = map.get(x, z);
    if y < height - 1 {
        return;
    }
    if kinds.contains(kind) {
        if y >= height {
            map.set(x, z, y + 1);
        }
        return;
    }
    if y != height - 1 {
        return;
    }
    let mut below = y - 1;
    while below >= floor {
        if predicates.get(read(below)).contains(kind) {
            map.set(x, z, below + 1);
            return;
        }
        below -= 1;
    }
    map.set(x, z, floor);
}
