use std::sync::Arc;

use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette, ChunkBlocks};
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_core::tag::TagPhase;
use mcrs_minecraft_core::tag::registry::DynTagRegistry;
use mcrs_minecraft_protocol::{BlockStateId, VarInt};
use mcrs_minecraft_world::block::Block;
use mcrs_minecraft_world::block::definition::{BlockStateFlags, Blocks};
use mcrs_minecraft_world::block::tags::{
    BLOCKS_MOTION_IN_HEIGHTMAP, BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES,
};
use mcrs_minecraft_world::transition_to_playing;
use mcrs_voxel_math::ColumnPos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_storage::{ColumnHeights, PalettedContainer, VoxelId};
use mcrs_voxel_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_voxel_world::world::storage::column::{ChunkLookup, ColumnChunks, ColumnIndex};
use rustc_hash::FxHashMap;

pub use mcrs_voxel_storage::ColumnHeights as ColumnHeightmap;

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

pub struct HeightmapPredicatesPlugin;

impl Plugin for HeightmapPredicatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            insert_heightmap_predicates
                .after(TagPhase::Freeze)
                .before(transition_to_playing),
        );
    }
}

fn insert_heightmap_predicates(
    mut commands: Commands,
    blocks: Res<Blocks>,
    tags: Res<DynTagRegistry<Block>>,
) {
    commands.insert_resource(heightmap_predicates(&blocks, &tags));
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

    fn set(&mut self, kinds: HeightmapKinds, x: usize, z: usize, y: i32) {
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
}

/// One descending pass over the column that closes every map at the first block
/// satisfying it.
///
/// A section whose palette is homogeneous settles all 256 columns from a single
/// table lookup, so the empty air above the terrain and the open water of an
/// ocean each cost one test rather than one per block.
pub fn build_column_heightmaps(
    sections: &[Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    predicates: &HeightmapPredicates,
) -> Option<ColumnHeightmapSet> {
    let first = *y_sections.first()?;
    let last = *y_sections.last()?;
    let min_y = first * BLOCKS::SIZE as i32;
    let height = ((last - first + 1) * BLOCKS::SIZE as i32) as u32;

    let mut set = ColumnHeightmapSet::new(height, min_y);
    let mut open = [HeightmapKinds::all(); BLOCKS::AREA];
    let mut remaining = BLOCKS::AREA;

    for (index, &section_y) in y_sections.iter().enumerate().rev() {
        if remaining == 0 {
            break;
        }
        let Some((blocks, _)) = sections.get(index).and_then(Option::as_ref) else {
            continue;
        };
        let section_min_y = section_y * BLOCKS::SIZE as i32;
        match &blocks.0 {
            PalettedContainer::Homogeneous(id) => {
                let kinds = predicates.get(*id);
                if kinds.is_empty() {
                    continue;
                }
                let top = section_min_y + BLOCKS::SIZE as i32;
                for cell in 0..BLOCKS::AREA {
                    remaining -= close(&mut set, &mut open[cell], cell, kinds, top) as usize;
                }
            }
            PalettedContainer::Heterogeneous(_) => {
                for local_y in (0..BLOCKS::SIZE).rev() {
                    if remaining == 0 {
                        break;
                    }
                    let top = section_min_y + local_y as i32 + 1;
                    for cell in 0..BLOCKS::AREA {
                        if open[cell].is_empty() {
                            continue;
                        }
                        let id = blocks
                            .0
                            .get(cell & BLOCKS::MASK, local_y, cell >> BLOCKS::BITS);
                        let kinds = predicates.get(id);
                        remaining -= close(&mut set, &mut open[cell], cell, kinds, top) as usize;
                    }
                }
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
    set.set(newly, cell & BLOCKS::MASK, cell >> BLOCKS::BITS, top);
    open.remove(newly);
    open.is_empty()
}

/// `Heightmap.Types` ids. The client reads the packet's map keys through them,
/// and takes only the three the server is expected to send.
const WORLD_SURFACE: i32 = 1;
const MOTION_BLOCKING: i32 = 4;
const MOTION_BLOCKING_NO_LEAVES: i32 = 5;

/// The three maps the chunk packet carries, in the client's wire encoding.
///
/// `SolidHeightmap` is deliberately absent: the client is never sent
/// `OCEAN_FLOOR`, and a map it does not ask for would only cost bandwidth.
pub fn client_heightmaps(
    surface: &SurfaceHeightmap,
    motion: &MotionHeightmap,
    no_leaves: &NoLeavesHeightmap,
) -> Vec<(VarInt, Vec<u64>)> {
    vec![
        (VarInt(WORLD_SURFACE), surface.0.raw_longs().to_vec()),
        (VarInt(MOTION_BLOCKING), motion.0.raw_longs().to_vec()),
        (
            VarInt(MOTION_BLOCKING_NO_LEAVES),
            no_leaves.0.raw_longs().to_vec(),
        ),
    ]
}

/// Maps built by the column task, waiting for their column entity to exist.
#[derive(Resource, Default, Debug)]
pub struct PendingColumnHeightmaps(pub FxHashMap<ColumnPos, ColumnHeightmapSet>);

pub struct DimHeightmapPlugin;

impl Plugin for DimHeightmapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingColumnHeightmaps>().add_systems(
            Last,
            update_column_heightmaps.run_if(resource_exists::<HeightmapPredicates>),
        );
    }
}

/// Stage 2.5: attach the freshly built maps to the column entity Stage 1 and 2
/// have just reconciled.
///
/// Merged with `max` because a column may be generated in more than one Y range:
/// "topmost block satisfying P" combines that way whatever order the ranges
/// arrive in, and a range that found nothing contributes its floor.
pub fn prime_column_heightmaps(
    mut pending: ResMut<PendingColumnHeightmaps>,
    indices: Query<&ColumnIndex>,
    dim_configs: Query<&DimensionTypeConfig>,
    columns: Query<(
        &InDimension,
        Option<(
            &SurfaceHeightmap,
            &SolidHeightmap,
            &MotionHeightmap,
            &NoLeavesHeightmap,
        )>,
    )>,
    mut commands: Commands,
) {
    if pending.0.is_empty() {
        return;
    }
    for (col, built) in pending.0.drain() {
        let Some(entity) = indices
            .iter()
            .find_map(|index| index.get(&col).map(|slot| slot.entity))
        else {
            continue;
        };
        let Ok((in_dim, existing)) = columns.get(entity) else {
            continue;
        };
        let Ok(config) = dim_configs.get(in_dim.0) else {
            continue;
        };
        let mut set = match existing {
            Some((surface, solid, motion, no_leaves)) => ColumnHeightmapSet {
                surface: surface.clone(),
                solid: solid.clone(),
                motion: motion.clone(),
                no_leaves: no_leaves.clone(),
            },
            None => ColumnHeightmapSet::new(config.height, config.min_y),
        };
        merge_max(&mut set.surface.0, &built.surface.0);
        merge_max(&mut set.solid.0, &built.solid.0);
        merge_max(&mut set.motion.0, &built.motion.0);
        merge_max(&mut set.no_leaves.0, &built.no_leaves.0);
        commands.entity(entity).insert(set);
    }
}

fn merge_max(into: &mut ColumnHeights, from: &ColumnHeights) {
    for z in 0..BLOCKS::SIZE {
        for x in 0..BLOCKS::SIZE {
            let candidate = from.get(x, z);
            // A found block always lands strictly above the range's own floor,
            // so the floor itself only ever means "nothing here" — and a range
            // starting above the dimension floor must not claim that as a height.
            if candidate == from.min_y() {
                continue;
            }
            let candidate = candidate.clamp(into.min_y(), into.max_y());
            if candidate > into.get(x, z) {
                into.set(x, z, candidate);
            }
        }
    }
}

/// Keeps the four maps exact across block edits, per the update rules: a write
/// strictly below the topmost satisfying block can neither raise nor lower the
/// map, and only removing that very block costs a descent.
pub fn update_column_heightmaps(
    mut placed: MessageReader<BlockPlaced>,
    predicates: Res<HeightmapPredicates>,
    indices: Query<&ColumnIndex>,
    mut columns: Query<(
        &ColumnChunks,
        &mut SurfaceHeightmap,
        &mut SolidHeightmap,
        &mut MotionHeightmap,
        &mut NoLeavesHeightmap,
    )>,
    palettes: Query<&ChunkBlocks>,
) {
    for edit in placed.read() {
        let col = ColumnPos::from(edit.block_pos);
        let Some(entity) = indices
            .iter()
            .find_map(|index| index.get(&col).map(|slot| slot.entity))
        else {
            continue;
        };
        let Ok((chunks, mut surface, mut solid, mut motion, mut no_leaves)) =
            columns.get_mut(entity)
        else {
            continue;
        };
        let x = (edit.block_pos.x & BLOCKS::MASK as i32) as usize;
        let z = (edit.block_pos.z & BLOCKS::MASK as i32) as usize;
        let y = edit.block_pos.y;
        let kinds = predicates.get(edit.new_state);
        let read = |at: i32| block_at(chunks, &palettes, x, at, z);

        let floor = surface.0.min_y();
        apply_edit(
            &mut surface.0,
            HeightmapKinds::SURFACE,
            x,
            z,
            y,
            kinds,
            floor,
            &predicates,
            &read,
        );
        apply_edit(
            &mut solid.0,
            HeightmapKinds::SOLID,
            x,
            z,
            y,
            kinds,
            floor,
            &predicates,
            &read,
        );
        apply_edit(
            &mut no_leaves.0,
            HeightmapKinds::NO_LEAVES,
            x,
            z,
            y,
            kinds,
            floor,
            &predicates,
            &read,
        );
        // The descent for MOTION stops at SOLID, whose block satisfies MOTION
        // too — so SOLID must already carry this edit's value.
        let solid_floor = solid.0.get(x, z);
        apply_edit(
            &mut motion.0,
            HeightmapKinds::MOTION,
            x,
            z,
            y,
            kinds,
            solid_floor,
            &predicates,
            &read,
        );
    }
}

#[allow(clippy::too_many_arguments)]
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

fn block_at(
    chunks: &ColumnChunks,
    palettes: &Query<&ChunkBlocks>,
    x: usize,
    y: i32,
    z: usize,
) -> VoxelId {
    let ChunkLookup::Loaded(section) = chunks.lookup(y.div_euclid(BLOCKS::SIZE as i32)) else {
        return VoxelId::default();
    };
    match palettes.get(section) {
        Ok(blocks) => blocks
            .get_cell(x, y.rem_euclid(BLOCKS::SIZE as i32) as usize, z),
        Err(_) => VoxelId::default(),
    }
}

#[cfg(test)]
mod tests;
