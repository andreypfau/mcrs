use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Query;
use bevy_math::DVec3;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkIndex, ChunkPos};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

const AIR: BlockStateId = BlockStateId(0);

/// Scale the source position by the nether coordinate divisor (8) and scan the
/// target's live blocks upward to find a safe landing position.
///
/// Reads blocks via `BlockPalette::get` through the `ChunkIndex` of `dim_entity`.
/// If the target chunk is not loaded the function falls back to a deterministic
/// position at the scaled coordinate so callers never panic.
pub fn find_or_create_nether_landing(
    dim_entity: Entity,
    source_pos: BlockPos,
    chunk_index_query: &Query<&ChunkIndex>,
    palette_query: &Query<&BlockPalette>,
) -> DVec3 {
    let scaled_x = source_pos.x / 8;
    let scaled_z = source_pos.z / 8;
    let base_y = 64i32;

    let chunk_index = match chunk_index_query.get(dim_entity) {
        Ok(ci) => ci,
        Err(_) => {
            return DVec3::new(scaled_x as f64, base_y as f64, scaled_z as f64);
        }
    };

    for scan_y in base_y..128 {
        let foot = BlockPos::new(scaled_x, scan_y, scaled_z);
        let head = BlockPos::new(scaled_x, scan_y + 1, scaled_z);

        if let Some(foot_state) = read_block(foot, chunk_index, palette_query) {
            if foot_state != AIR {
                let above = BlockPos::new(scaled_x, scan_y + 1, scaled_z);
                let above2 = BlockPos::new(scaled_x, scan_y + 2, scaled_z);
                let s1 = read_block(above, chunk_index, palette_query).unwrap_or(AIR);
                let s2 = read_block(above2, chunk_index, palette_query).unwrap_or(AIR);
                if s1 == AIR && s2 == AIR {
                    return DVec3::new(scaled_x as f64, (scan_y + 1) as f64, scaled_z as f64);
                }
            }
        } else {
            return DVec3::new(scaled_x as f64, base_y as f64, scaled_z as f64);
        }
        let _ = head;
    }

    DVec3::new(scaled_x as f64, base_y as f64, scaled_z as f64)
}

fn read_block(
    pos: BlockPos,
    chunk_index: &ChunkIndex,
    palette_query: &Query<&BlockPalette>,
) -> Option<BlockStateId> {
    let chunk_entity = chunk_index.get(ChunkPos::from(pos))?;
    let palette = palette_query.get(chunk_entity).ok()?;
    Some(palette.get(pos))
}
