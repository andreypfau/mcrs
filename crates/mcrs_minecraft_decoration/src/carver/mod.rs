pub mod cave;
pub mod config;
pub mod mask;
pub mod water;

use crate::carver::config::BetaCaveCarverConfig;
use crate::carver::mask::CarvingMask;
use crate::carver::water::{WaterMask, water_abort_scan};
use mcrs_minecraft_random::Random;
use mcrs_voxel_storage::VoxelId;

pub trait WorldCarver {
    fn carve<R: Random>(
        &self,
        config: &BetaCaveCarverConfig,
        chunk_x: i32,
        chunk_z: i32,
        origin_x: i32,
        origin_z: i32,
        water: &WaterMask,
        mask: &mut CarvingMask,
        rng: &mut R,
    );
}

pub fn can_replace_block(config: &BetaCaveCarverConfig, state: VoxelId) -> bool {
    state == config.stone_state || state == config.dirt_state || state == config.grass_state
}

/// Mark the ellipsoid at (d0, d1, d2), horizontal radius d6 and vertical radius
/// d7, as carved in `mask`, in chunk-local coordinates.
///
/// The marked Y is the one the ellipsoid test accepts; Beta writes the block one
/// above it, which is the substance pass's business, not this one's.
///
/// Returns false if carving was aborted because water touches the shell.
pub fn carve_ellipsoid(
    chunk_x: i32,
    chunk_z: i32,
    d0: f64,
    d1: f64,
    d2: f64,
    d6: f64,
    d7: f64,
    water: &WaterMask,
    mask: &mut CarvingMask,
) -> bool {
    let k1 = ((d0 - d6).floor() as i32 - chunk_x * 16 - 1).max(0);
    let l1 = ((d0 + d6).floor() as i32 - chunk_x * 16 + 1).min(16);
    let i2 = ((d1 - d7).floor() as i32 - 1).max(1);
    let j2 = ((d1 + d7).floor() as i32 + 1).min(120);
    let k2 = ((d2 - d6).floor() as i32 - chunk_z * 16 - 1).max(0);
    let l2 = ((d2 + d6).floor() as i32 - chunk_z * 16 + 1).min(16);

    if water_abort_scan(water, k1, l1, i2, j2, k2, l2) {
        return false;
    }

    for lx in k1..l1 {
        let d12 = ((lx + chunk_x * 16) as f64 + 0.5 - d0) / d6;
        for lz in k2..l2 {
            let d13 = ((lz + chunk_z * 16) as f64 + 0.5 - d2) / d6;
            if d12 * d12 + d13 * d13 >= 1.0 {
                continue;
            }
            for world_y in i2..j2 {
                let d14 = (world_y as f64 + 0.5 - d1) / d7;
                if d14 > -0.7 && d12 * d12 + d14 * d14 + d13 * d13 < 1.0 {
                    mask.carve(lx, world_y, lz);
                }
            }
        }
    }
    true
}
