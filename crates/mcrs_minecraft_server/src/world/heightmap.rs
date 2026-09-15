use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::tag::TagPhase;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_block::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{ColumnHeights, VoxelId};
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::block_update::BlockPlaced;
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_level::world::storage::column::{ChunkLookup, ColumnChunks, ColumnIndex};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_world::transition_to_playing;
use mcrs_minecraft_worldgen_generator::heightmap::{
    ColumnHeightmapSet, HeightmapPredicates, MotionHeightmap, NoLeavesHeightmap, SolidHeightmap,
    SurfaceHeightmap, apply_write, heightmap_predicates,
};
use rustc_hash::FxHashMap;

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
    for z in 0..SectionPos::SIZE {
        for x in 0..SectionPos::SIZE {
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
        let x = (edit.block_pos.x & SectionPos::MASK as i32) as usize;
        let z = (edit.block_pos.z & SectionPos::MASK as i32) as usize;
        let y = edit.block_pos.y;
        let kinds = predicates.get(edit.new_state);
        let read = |at: i32| block_at(chunks, &palettes, x, at, z);

        apply_write(
            &mut surface.0,
            &mut solid.0,
            &mut motion.0,
            &mut no_leaves.0,
            x,
            z,
            y,
            kinds,
            &predicates,
            &read,
        );
    }
}

fn block_at(
    chunks: &ColumnChunks,
    palettes: &Query<&ChunkBlocks>,
    x: usize,
    y: i32,
    z: usize,
) -> VoxelId {
    let ChunkLookup::Loaded(section) = chunks.lookup(y.div_euclid(SectionPos::SIZE as i32)) else {
        return VoxelId::default();
    };
    match palettes.get(section) {
        Ok(blocks) => blocks.get_cell(x, y.rem_euclid(SectionPos::SIZE as i32) as usize, z),
        Err(_) => VoxelId::default(),
    }
}

#[cfg(test)]
mod tests;
