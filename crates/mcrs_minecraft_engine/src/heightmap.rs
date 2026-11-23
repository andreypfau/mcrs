use bevy_app::{App, Last, Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{ColumnHeights, VoxelId};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_heightmap::{
    ColumnHeightmapSet, HeightmapPredicates, MotionHeightmap, NoLeavesHeightmap, SolidHeightmap,
    SurfaceHeightmap, apply_write, heightmap_predicates,
};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

#[derive(Resource, Clone)]
pub struct BlockHeightmapPredicates(pub HeightmapPredicates);

pub struct HeightmapPredicatesPlugin;

impl Plugin for HeightmapPredicatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            rebuild_heightmap_predicates.run_if(resource_exists_and_changed::<CurrentRegistries>),
        );
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::per_frame_insert_resource,
        reason = "runs only when the registries change, and it replaces the predicates with the ones of the new registries"
    )
)]
fn rebuild_heightmap_predicates(
    mut commands: Commands,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
) {
    let Some(tags) = registries.tags::<Block>() else {
        tracing::error!(
            "the registries declare no block registry with its tags; the heightmap predicates stay as they were"
        );
        return;
    };
    commands.insert_resource(BlockHeightmapPredicates(heightmap_predicates(
        &blocks, &tags,
    )));
}

const WORLD_SURFACE: i32 = 1;

const MOTION_BLOCKING: i32 = 4;

const MOTION_BLOCKING_NO_LEAVES: i32 = 5;

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

#[derive(Resource, Default, Debug)]
pub struct PendingColumnHeightmaps(pub FxHashMap<(Entity, ColumnPos), ColumnHeightmapSet>);

pub struct DimHeightmapPlugin;

impl Plugin for DimHeightmapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingColumnHeightmaps>().add_systems(
            Last,
            update_column_heightmaps.run_if(resource_exists::<BlockHeightmapPredicates>),
        );
    }
}

pub fn prime_column_heightmaps(
    mut pending: ResMut<PendingColumnHeightmaps>,
    indices: Query<&ColumnIndex>,
    dim_configs: Query<&DimensionTypeConfig>,
    columns: Query<
        Option<(
            &SurfaceHeightmap,
            &SolidHeightmap,
            &MotionHeightmap,
            &NoLeavesHeightmap,
        )>,
    >,
    mut commands: Commands,
) {
    if pending.0.is_empty() {
        return;
    }
    for ((dimension, col), built) in pending.0.drain() {
        let Some(entity) = indices
            .get(dimension)
            .ok()
            .and_then(|index| index.get(&col))
            .map(|slot| slot.entity)
        else {
            continue;
        };
        let Ok(existing) = columns.get(entity) else {
            continue;
        };
        let Ok(config) = dim_configs.get(dimension) else {
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
    for z in 0..SectionPos::SIZE {
        for x in 0..SectionPos::SIZE {
            let candidate = from.get(x, z);
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

pub fn update_column_heightmaps(
    mut placed: MessageReader<BlockPlaced>,
    predicates: Res<BlockHeightmapPredicates>,
    sections: Query<&InDimension>,
    indices: Query<(&ColumnIndex, &SectionIndex)>,
    mut columns: Query<(
        &mut SurfaceHeightmap,
        &mut SolidHeightmap,
        &mut MotionHeightmap,
        &mut NoLeavesHeightmap,
    )>,
) {
    for edit in placed.read() {
        let Ok(&InDimension(dimension)) = sections.get(edit.chunk) else {
            continue;
        };
        let col = ColumnPos::from(edit.block_pos);
        let Ok((column_index, section_index)) = indices.get(dimension) else {
            continue;
        };
        let Some(entity) = column_index.get(&col).map(|slot| slot.entity) else {
            continue;
        };
        let Ok((mut surface, mut solid, mut motion, mut no_leaves)) = columns.get_mut(entity)
        else {
            continue;
        };
        let x = (edit.block_pos.x & SectionPos::MASK as i32) as usize;
        let z = (edit.block_pos.z & SectionPos::MASK as i32) as usize;
        let y = edit.block_pos.y;
        let kinds = predicates.0.get(edit.new_state.into());
        let read = |at: i32| block_at(section_index, col, x, at, z);

        apply_write(
            &mut surface.0,
            &mut solid.0,
            &mut motion.0,
            &mut no_leaves.0,
            x,
            z,
            y,
            kinds,
            &predicates.0,
            &read,
        );
    }
}

fn block_at(index: &SectionIndex, col: ColumnPos, x: usize, y: i32, z: usize) -> VoxelId {
    let section = SectionPos::new(col.x, y.div_euclid(SectionPos::SIZE as i32), col.z);
    index
        .blocks(section)
        .and_then(|blocks| blocks.get_cell(x, y.rem_euclid(SectionPos::SIZE as i32) as usize, z))
        .map_or_else(VoxelId::default, Into::into)
}

#[cfg(test)]
mod tests;
