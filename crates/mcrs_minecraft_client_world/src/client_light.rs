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

pub(crate) fn relight(
    store: &mut ColumnStore,
    registry: &Arc<LightRegistry>,
    checks: &[LightCheck],
) {
    let Some(dimension) = store.dimension() else {
        return;
    };
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

const TILE_BITS: u32 = 2;

const JOB_COLUMNS: usize = 256;

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
