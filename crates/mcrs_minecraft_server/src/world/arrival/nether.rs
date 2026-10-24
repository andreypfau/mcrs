use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_level::world::storage::view::WorldView;
use mcrs_minecraft_volume::Cursor;

/// Scale a horizontal overworld coordinate into the nether by the vanilla
/// divisor of 8. Uses floor division so negative coordinates round toward
/// negative infinity — a truncating `/` rounds toward zero and lands portals
/// one column off on the negative side of each axis.
fn nether_scale(coord: i32) -> i32 {
    coord.div_euclid(8)
}

/// Scale the source position by the nether coordinate divisor (8) and scan the
/// target's live blocks upward to find a safe landing position.
///
/// If the target section is not loaded the function falls back to a deterministic
/// position at the scaled coordinate so callers never panic.
pub fn find_or_create_nether_landing(
    dim_entity: Entity,
    source_pos: BlockPos,
    blocks: &WorldView,
) -> DVec3 {
    let scaled_x = nether_scale(source_pos.x);
    let scaled_z = nether_scale(source_pos.z);
    let base_y = 64i32;
    let fallback = DVec3::new(scaled_x as f64, base_y as f64, scaled_z as f64);

    let Some(dimension) = blocks.get(dim_entity) else {
        return fallback;
    };
    let mut cursor = Cursor::new(&dimension);
    let mut block = |y: i32| cursor.block(BlockPos::new(scaled_x, y, scaled_z));

    for scan_y in base_y..128 {
        let Ok(foot) = block(scan_y) else {
            return fallback;
        };
        if foot != BlockStateId::AIR {
            let s1 = block(scan_y + 1).unwrap_or(BlockStateId::AIR);
            let s2 = block(scan_y + 2).unwrap_or(BlockStateId::AIR);
            if s1 == BlockStateId::AIR && s2 == BlockStateId::AIR {
                return DVec3::new(scaled_x as f64, (scan_y + 1) as f64, scaled_z as f64);
            }
        }
    }

    fallback
}

#[cfg(test)]
mod tests {
    use super::nether_scale;

    #[test]
    fn scales_negative_coords_toward_negative_infinity() {
        assert_eq!(nether_scale(16), 2);
        assert_eq!(nether_scale(7), 0);
        // A truncating `/ 8` would give 0 and -1 here — the bug this guards.
        assert_eq!(nether_scale(-1), -1);
        assert_eq!(nether_scale(-9), -2);
    }
}
