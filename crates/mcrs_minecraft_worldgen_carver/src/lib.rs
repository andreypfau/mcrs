#[rustfmt::skip]
pub mod carver_type_keys;
#[rustfmt::skip]
pub mod registry_keys;
pub mod beta;
pub mod canyon;
pub mod config;
pub mod mask;
pub mod modern;
pub mod target;
pub mod tunnel;
pub mod water;

use crate::mask::CarvingMask;
use crate::water::{WaterMask, water_abort_scan};

#[derive(Clone, Copy)]
pub enum CarveShape<'a> {
    Cave {
        floor_level: f64,
    },
    Canyon {
        width_factors: &'a [f32],
        min_y: i32,
    },
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::too_many_arguments,
        reason = "independent inputs of one ellipsoid carve"
    )
)]
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
                        let Some(&width) = width_factors.get((world_y - anchor - 1) as usize)
                        else {
                            continue;
                        };
                        let width = width as f64;
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

mcrs_minecraft_registries::resolve_as_is!(crate::config::CarverConfig);
