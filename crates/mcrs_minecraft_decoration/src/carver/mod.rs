pub mod beta;
pub mod canyon;
pub mod config;
pub mod mask;
pub mod modern;
pub mod tunnel;
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

/// Which cells of the bounding box a carver's cross-section leaves out.
///
/// Java hands the rasteriser a `CarveSkipChecker` lambda per ellipsoid. Naming
/// the two shapes instead keeps the test out of a closure call: the rasteriser
/// matches once per ellipsoid and runs a loop that has no indirection in it.
#[derive(Clone, Copy)]
pub enum CarveShape<'a> {
    /// An ellipsoid cut off below `floor_level`. Both cave carvers.
    Cave { floor_level: f64 },
    /// A cross-section modulated by the canyon's per-height width table, with
    /// the vertical stretched by the `yd * yd / 6` term.
    Canyon {
        width_factors: &'a [f32],
        min_y: i32,
    },
}

/// Mark the ellipsoid at (x, y, z), horizontal radius `horizontal_radius` and
/// vertical radius `vertical_radius`, as carved in `mask`.
///
/// Returns false if nothing was rasterised: either the ellipsoid misses the
/// target chunk entirely, or water touches the shell of its bounds and Beta's
/// abort applies. Modern carvers pass an empty water mask and never abort.
#[allow(clippy::too_many_arguments)]
pub fn carve_ellipsoid(
    chunk_x: i32,
    chunk_z: i32,
    x: f64,
    y: f64,
    z: f64,
    horizontal_radius: f64,
    vertical_radius: f64,
    shape: CarveShape<'_>,
    water: &WaterMask,
    mask: &mut CarvingMask,
) -> bool {
    let center_x = chunk_x as f64 * 16.0 + 8.0;
    let center_z = chunk_z as f64 * 16.0 + 8.0;
    let max_delta = 16.0 + horizontal_radius * 2.0;
    if (x - center_x).abs() > max_delta || (z - center_z).abs() > max_delta {
        return false;
    }

    let min_x = ((x - horizontal_radius).floor() as i32 - chunk_x * 16 - 1).max(0);
    let max_x = ((x + horizontal_radius).floor() as i32 - chunk_x * 16).min(15);
    let min_y = ((y - vertical_radius).floor() as i32 - 1).max(mask.min_y());
    let max_y = ((y + vertical_radius).floor() as i32 + 1).min(mask.max_y());
    let min_z = ((z - horizontal_radius).floor() as i32 - chunk_z * 16 - 1).max(0);
    let max_z = ((z + horizontal_radius).floor() as i32 - chunk_z * 16).min(15);

    if water_abort_scan(water, min_x, max_x + 1, min_y, max_y, min_z, max_z + 1) {
        return false;
    }

    match shape {
        CarveShape::Cave { floor_level } => {
            for local_x in min_x..=max_x {
                let xd = ((local_x + chunk_x * 16) as f64 + 0.5 - x) / horizontal_radius;
                for local_z in min_z..=max_z {
                    let zd = ((local_z + chunk_z * 16) as f64 + 0.5 - z) / horizontal_radius;
                    if xd * xd + zd * zd >= 1.0 {
                        continue;
                    }
                    for world_y in (min_y + 1..=max_y).rev() {
                        let yd = (world_y as f64 - 0.5 - y) / vertical_radius;
                        if yd > floor_level && xd * xd + yd * yd + zd * zd < 1.0 {
                            mask.carve(local_x, world_y, local_z);
                        }
                    }
                }
            }
        }
        CarveShape::Canyon {
            width_factors,
            min_y: anchor,
        } => {
            for local_x in min_x..=max_x {
                let xd = ((local_x + chunk_x * 16) as f64 + 0.5 - x) / horizontal_radius;
                for local_z in min_z..=max_z {
                    let zd = ((local_z + chunk_z * 16) as f64 + 0.5 - z) / horizontal_radius;
                    let horizontal = xd * xd + zd * zd;
                    if horizontal >= 1.0 {
                        continue;
                    }
                    for world_y in (min_y + 1..=max_y).rev() {
                        let yd = (world_y as f64 - 0.5 - y) / vertical_radius;
                        let width = width_factors[(world_y - anchor - 1) as usize] as f64;
                        if horizontal * width + yd * yd / 6.0 < 1.0 {
                            mask.carve(local_x, world_y, local_z);
                        }
                    }
                }
            }
        }
    }
    true
}
