use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys::{
    BLOCKS_MOTION_IN_HEIGHTMAP, BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES,
};
use mcrs_minecraft_block::definition::{BlockStateFlags, Blocks};
use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_chunk::{ColumnHeights, PalettedContainer, VoxelId};
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::TagId;
use mcrs_voxel::{LocalPos, SectionPos};
use std::cell::Cell;
use std::sync::Arc;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct SurfaceHeightmap(pub ColumnHeights);

#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct SolidHeightmap(pub ColumnHeights);

#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct MotionHeightmap(pub ColumnHeights);

#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct NoLeavesHeightmap(pub ColumnHeights);

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct HeightmapKinds: u8 {
        const SURFACE = 1 << 0;
        const SOLID = 1 << 1;
        const MOTION = 1 << 2;
        const NO_LEAVES = 1 << 3;
    }
}

#[derive(Clone, Debug)]
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

pub fn heightmap_predicates(blocks: &Blocks, tags: &Tags<Block>) -> HeightmapPredicates {
    let motion = tags.id(BLOCKS_MOTION_IN_HEIGHTMAP);
    let motion_no_leaves = tags.id(BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES);
    let has = |tag: Option<TagId<Block>>, block: Option<_>| {
        tag.zip(block)
            .is_some_and(|(tag, block)| tags.contains(tag, block))
    };
    let mut table = Vec::with_capacity(blocks.state_count());
    for id in blocks.states() {
        let state = blocks.state(id);
        let block = blocks.block_index(id);
        let mut kinds = HeightmapKinds::empty();
        if !state.flags.contains(BlockStateFlags::IS_AIR) {
            kinds |= HeightmapKinds::SURFACE;
        }
        if has(motion, block) {
            kinds |= HeightmapKinds::SOLID | HeightmapKinds::MOTION;
        }
        if has(motion_no_leaves, block) {
            kinds |= HeightmapKinds::NO_LEAVES;
        }
        if state.fluid.is_some() {
            kinds |= HeightmapKinds::MOTION | HeightmapKinds::NO_LEAVES;
        }
        table.push(kinds);
    }
    HeightmapPredicates(table.into())
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::bundle::Bundle))]
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::too_many_arguments,
        reason = "independent inputs of one block write"
    )
)]
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

pub fn build_dense_heightmaps<'a>(
    y_sections: &[i32],
    wanted: HeightmapKinds,
    section: impl Fn(usize) -> &'a [Cell<VoxelId>],
    predicates: &HeightmapPredicates,
) -> Option<ColumnHeightmapSet> {
    if (0..y_sections.len()).any(|index| section(index).len() != SectionPos::VOLUME) {
        return None;
    }
    descend(
        y_sections,
        wanted,
        |index| Some(SectionCells::Dense(section(index))),
        predicates,
    )
}

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
            SectionCells::Dense(cells) => cells
                .get(local_y * SectionPos::AREA + cell)
                .map(Cell::get)
                .unwrap_or_default(),
            SectionCells::Palette(palette) => palette.get_local(LocalPos::new(
                (cell & SectionPos::MASK) as u8,
                local_y as u8,
                (cell >> SectionPos::BITS) as u8,
            )),
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::too_many_arguments,
        reason = "independent inputs of one block write"
    )
)]
fn apply_edit(
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
