use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_light_engine::SectionBlocks;
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::level::LightBounds;
use mcrs_minecraft_light_engine::world::{Edit, LightWorld};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::columns::{AIR, ColumnStore, Columns, LightCheck, SectionLightChange};

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
    let checks: Vec<LightCheck> = checks
        .iter()
        .copied()
        .filter(|check| registry.light_properties_differ(check.old.into(), check.new.into()))
        .collect();
    let Some(dimension) = store.dimension() else {
        return;
    };
    if checks.is_empty() {
        return;
    }
    let extent = dimension.extent;
    let max_section_y = extent.min_section_y + extent.sections as i32 - 1;
    let bounds = LightBounds::new(extent.min_section_y, max_section_y);
    let world = LightWorld::new(Arc::clone(registry), bounds);
    let mut world = if dimension.sky_light {
        world
    } else {
        world.without_sky()
    };

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
    columns.retain(|&column| store.holds(column));

    let section_ys = extent.min_section_y..=max_section_y;
    let mut loads = Vec::new();
    for &column in &columns {
        for sy in section_ys.clone() {
            let at = SectionPos::new(column.x, sy, column.z);
            loads.push(Edit::LoadSection {
                pos: at,
                entity: 0,
                blocks: Arc::new(blocks_before(store, at, &first_old)),
            });
        }
    }
    world.apply_edits(loads);
    for &column in &columns {
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
