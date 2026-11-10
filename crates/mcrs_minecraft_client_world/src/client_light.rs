use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::RangeInclusive;
use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_light_engine::SectionBlocks;
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::level::LightBounds;
use mcrs_minecraft_light_engine::world::{Edit, LightWorld};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::columns::{
    AIR, ColumnStore, Columns, Dimension, LightCheck, SectionLightChange, UnlitColumnLight,
};

/// Lights the block changes of `checks` again in the columns the store holds, from the light it
/// holds, and writes back each section whose light moved.
///
/// The light engine keeps its own copy of the blocks, so the step builds one for the columns
/// that a change can reach: a change moves light at most 15 blocks, which is never past the
/// neighbour column. Each column comes whole, because its sky scan reads it from the top.
pub(crate) fn relight(
    store: &mut ColumnStore,
    registry: &Arc<LightRegistry>,
    checks: &[LightCheck],
) {
    let Some(dimension) = store.dimension() else {
        return;
    };
    // A column that waits to be lit from its blocks takes these changes with it, and its
    // light is not the light of the change.
    let checks: Vec<LightCheck> = checks
        .iter()
        .copied()
        .filter(|check| registry.light_properties_differ(check.old.into(), check.new.into()))
        .filter(|check| !store.is_unlit_arrival(ColumnPos::from(check.pos)))
        .collect();
    if checks.is_empty() {
        return;
    }
    let (mut world, section_ys) = engine_world(registry, dimension, true);

    let mut first_old: HashMap<BlockPos, BlockStateId> = HashMap::new();
    for check in &checks {
        first_old.entry(check.pos).or_insert(check.old);
    }
    let mut columns: HashSet<ColumnPos> = HashSet::new();
    for check in &checks {
        let at = ColumnPos::from(check.pos);
        for dz in -1..=1 {
            for dx in -1..=1 {
                columns.insert(ColumnPos::new(at.x + dx, at.z + dz));
            }
        }
    }
    columns.retain(|&column| store.holds(column) && !store.is_unlit_arrival(column));

    load_held(&mut world, store, &columns, &section_ys, &first_old);

    let work = world.apply_edits(checks.iter().map(|check| Edit::SetBlock {
        pos: check.pos,
        block: check.new,
    }));
    let Some(job) = world.prepare_batch(work.into_iter().map(|(_, influence)| influence)) else {
        return;
    };
    for section in job.run().sections {
        store.set_section_light(
            section.pos,
            SectionLightChange {
                sky: section.sky_light.as_ref(),
                block: section.block_light.as_ref(),
            },
        );
    }
}

/// Columns per side of one tile of `light_arrived`: a job covers whole tiles and the ring around
/// them, so a scattered batch never grows one job to the box that holds all of it.
const TILE_BITS: u32 = 2;

/// The most columns a job of `light_arrived` takes from a batch before it stops adding the tiles
/// beside it, which keeps the memory of one engine world bounded. A job may pass it by the
/// tiles it was adding.
const JOB_COLUMNS: usize = 256;

/// An engine world for the dimension, empty, and the section rows of its extent. The sky layer
/// is only calculated when `sky` asks for it and the dimension has sky light.
fn engine_world(
    registry: &Arc<LightRegistry>,
    dimension: &Dimension,
    sky: bool,
) -> (LightWorld, RangeInclusive<i32>) {
    let extent = dimension.extent;
    let max_section_y = extent.min_section_y + extent.sections as i32 - 1;
    let world = LightWorld::new(
        Arc::clone(registry),
        LightBounds::new(extent.min_section_y, max_section_y),
    );
    let world = if sky && dimension.sky_light {
        world
    } else {
        world.without_sky()
    };
    (world, extent.min_section_y..=max_section_y)
}

/// Lights the columns that arrived without light, from their blocks, and lets the light cross
/// into the columns the store holds around them. Unless the mode computes the sky layer too, that
/// layer is neither computed nor written, so it stays as the embedder set it.
///
/// A column that no job has lit yet is dark and its own job is still to come, so the jobs before
/// it neither read nor write it.
pub(crate) fn light_arrived(
    store: &mut ColumnStore,
    registry: &Arc<LightRegistry>,
    arrived: &[ColumnPos],
    mode: UnlitColumnLight,
) {
    let sky = match mode {
        UnlitColumnLight::Off => return,
        UnlitColumnLight::Block => false,
        UnlitColumnLight::BlockAndSky => true,
    };
    let Some(dimension) = store.dimension() else {
        return;
    };
    let dimension = dimension.clone();

    let mut tiles: BTreeMap<(i32, i32), Vec<ColumnPos>> = BTreeMap::new();
    for &column in arrived {
        if store.holds(column) {
            tiles
                .entry((column.x >> TILE_BITS, column.z >> TILE_BITS))
                .or_default()
                .push(column);
        }
    }

    let mut queued: HashSet<ColumnPos> = tiles.values().flatten().copied().collect();
    while let Some(group) = take_group(&mut tiles) {
        for column in &group {
            queued.remove(column);
        }

        let (mut world, section_ys) = engine_world(registry, &dimension, sky);
        let mut ring: HashSet<ColumnPos> = HashSet::new();
        for column in &group {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let around = ColumnPos::new(column.x + dx, column.z + dz);
                    if !group.contains(&around)
                        && !queued.contains(&around)
                        && !store.is_unlit_arrival(around)
                        && store.holds(around)
                    {
                        ring.insert(around);
                    }
                }
            }
        }
        let no_changes = HashMap::new();
        load_held(&mut world, store, &ring, &section_ys, &no_changes);

        // The ring is loaded already, so only the group's own sections owe a first answer
        // and carry the work.
        let mut loads = Vec::new();
        for &column in &group {
            loads.extend(load_edits(store, column, &section_ys, &no_changes));
        }
        let work = world.apply_edits(loads);
        let Some(job) = world.prepare_batch(work.into_iter().map(|(_, influence)| influence))
        else {
            continue;
        };
        for section in job.run().sections {
            store.set_section_light(
                section.pos,
                SectionLightChange {
                    // A world without sky still answers a first sky layer of zeros, which
                    // would wipe the sky the embedder set.
                    sky: if sky && dimension.sky_light {
                        section.sky_light.as_ref()
                    } else {
                        None
                    },
                    block: section.block_light.as_ref(),
                },
            );
        }
    }
}

/// Takes the next group of touching tiles, up to `JOB_COLUMNS` columns, so that one job lights
/// them together.
fn take_group(tiles: &mut BTreeMap<(i32, i32), Vec<ColumnPos>>) -> Option<HashSet<ColumnPos>> {
    let (seed, columns) = tiles.pop_first()?;
    let mut group: HashSet<ColumnPos> = columns.into_iter().collect();
    let mut frontier = vec![seed];
    while group.len() < JOB_COLUMNS {
        let Some((tx, tz)) = frontier.pop() else {
            break;
        };
        for dz in -1..=1 {
            for dx in -1..=1 {
                let next = (tx + dx, tz + dz);
                if let Some(columns) = tiles.remove(&next) {
                    group.extend(columns);
                    frontier.push(next);
                }
            }
        }
    }
    Some(group)
}

/// Loads the sections of `columns` into the engine with the light the store holds for them.
fn load_held(
    world: &mut LightWorld,
    store: &ColumnStore,
    columns: &HashSet<ColumnPos>,
    section_ys: &RangeInclusive<i32>,
    first_old: &HashMap<BlockPos, BlockStateId>,
) {
    let loads: Vec<Edit> = columns
        .iter()
        .flat_map(|&column| load_edits(store, column, section_ys, first_old))
        .collect();
    world.apply_edits(loads);
    for &column in columns {
        for sy in section_ys.clone() {
            let at = SectionPos::new(column.x, sy, column.z);
            let (Some(light), Some(section)) = (store.section_light(at), world.section_mut(at))
            else {
                continue;
            };
            section.sky_light = light.sky;
            section.block_light = light.block;
            section.lit = true;
        }
    }
}

fn load_edits(
    store: &ColumnStore,
    column: ColumnPos,
    section_ys: &RangeInclusive<i32>,
    first_old: &HashMap<BlockPos, BlockStateId>,
) -> Vec<Edit> {
    section_ys
        .clone()
        .map(|sy| {
            let at = SectionPos::new(column.x, sy, column.z);
            Edit::LoadSection {
                pos: at,
                entity: 0,
                blocks: Arc::new(blocks_before(store, at, first_old)),
            }
        })
        .collect()
}

/// The blocks of a section as they stood before the first change of `first_old` in it.
fn blocks_before(
    store: &ColumnStore,
    at: SectionPos,
    first_old: &HashMap<BlockPos, BlockStateId>,
) -> SectionBlocks {
    let mut blocks = store
        .stored_section(at)
        .and_then(|section| VoxelPalette::from_cells(section.blocks.as_slice()))
        .unwrap_or_else(|| VoxelPalette::homogeneous(AIR));
    for (&pos, &old) in first_old {
        if SectionPos::from(pos) == at {
            blocks.set(LocalPos::from(pos), old);
        }
    }
    blocks
}
