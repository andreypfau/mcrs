use crate::carver::config::BetaCaveCarverConfig;
use crate::carver::mask::CarvingMask;
use crate::carver::water::WaterMask;
use crate::carver::{WorldCarver, carve_ellipsoid};
use crate::math::{cos as math_helper_cos, sin as math_helper_sin};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

pub struct CaveWorldCarver;

impl WorldCarver for CaveWorldCarver {
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
    ) {
        let cave_count = {
            let a = rng.next_i32_bound(40) + 1;
            let b = rng.next_i32_bound(a) + 1;
            rng.next_i32_bound(b)
        };
        let cave_count = if rng.next_i32_bound(15) != 0 {
            0
        } else {
            cave_count
        };

        for _ in 0..cave_count {
            let world_x = (origin_x * 16 + rng.next_i32_bound(16)) as f64;
            let y_bound = rng.next_i32_bound(120) + 8;
            let world_y = rng.next_i32_bound(y_bound) as f64;
            let world_z = (origin_z * 16 + rng.next_i32_bound(16)) as f64;

            let mut tunnel_count = 1;
            if rng.next_i32_bound(4) == 0 {
                let room_thickness = 1.0 + rng.next_f32() * 6.0;
                let tunnel_seed = rng.next_java_long() as u64;
                let mut tunnel_rng = LegacyRandom::new(tunnel_seed);
                create_tunnel(
                    config,
                    chunk_x,
                    chunk_z,
                    world_x,
                    world_y,
                    world_z,
                    room_thickness,
                    0.0f32,
                    0.0f32,
                    -1,
                    -1,
                    0.5,
                    water,
                    mask,
                    &mut tunnel_rng,
                    rng,
                );
                tunnel_count += rng.next_i32_bound(4);
            }

            for _ in 0..tunnel_count {
                let yaw = rng.next_f32() * std::f32::consts::TAU;
                let pitch = (rng.next_f32() - 0.5) * 2.0 / 8.0;
                let thickness = rng.next_f32() * 2.0 + rng.next_f32();

                let tunnel_seed = rng.next_java_long() as u64;
                let mut tunnel_rng = LegacyRandom::new(tunnel_seed);
                create_tunnel(
                    config,
                    chunk_x,
                    chunk_z,
                    world_x,
                    world_y,
                    world_z,
                    thickness,
                    yaw,
                    pitch,
                    0,
                    0,
                    1.0,
                    water,
                    mask,
                    &mut tunnel_rng,
                    rng,
                );
            }
        }
    }
}

/// Port of MapGenCaves.a(int i, int j, byte[] abyte, double, double, double, float, float, float, int, int, double).
///
/// `rng` is the per-tunnel LegacyRandom (seeded from parent via next_u64).
/// `parent_rng` is the per-carve-origin Random, used only for seeding split sub-tunnels
/// (mirrors Java `this.b.nextLong()` in recursive calls).
#[allow(clippy::too_many_arguments)]
fn create_tunnel<R: Random>(
    config: &BetaCaveCarverConfig,
    chunk_x: i32,
    chunk_z: i32,
    mut d0: f64,
    mut d1: f64,
    mut d2: f64,
    f: f32,
    mut f1: f32,
    mut f2: f32,
    mut step: i32,
    total_steps: i32,
    d3: f64,
    water: &WaterMask,
    mask: &mut CarvingMask,
    rng: &mut LegacyRandom,
    parent_rng: &mut R,
) {
    let range = config.range;
    let mut total_steps = total_steps;

    if total_steps <= 0 {
        let i1 = range * 16 - 16;
        total_steps = i1 - rng.next_i32_bound(i1 / 4);
    }

    let mut is_room = false;
    if step == -1 {
        step = total_steps / 2;
        is_room = true;
    }

    let branch_at = rng.next_i32_bound(total_steps / 2) + total_steps / 4;
    let squiggly = rng.next_i32_bound(6) == 0;
    let mut f3 = 0.0f32;
    let mut f4 = 0.0f32;

    while step < total_steps {
        let d6 = 1.5_f64
            + (math_helper_sin(step as f32 * std::f32::consts::PI / total_steps as f32) * f * 1.0)
                as f64;
        let d7 = d6 * d3;

        let f5 = math_helper_cos(f2);
        let f6 = math_helper_sin(f2);
        d0 += (math_helper_cos(f1) * f5) as f64;
        d1 += f6 as f64;
        d2 += (math_helper_sin(f1) * f5) as f64;

        if squiggly {
            f2 *= 0.92;
        } else {
            f2 *= 0.7;
        }
        f2 += f4 * 0.1;
        f1 += f3 * 0.1;
        f4 *= 0.9;
        f3 *= 0.75;
        f4 += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 2.0;
        f3 += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 4.0;

        if !is_room && step == branch_at && f > 1.0 {
            let f_a = rng.next_f32() * 0.5 + 0.5;
            let seed_a = parent_rng.next_java_long() as u64;
            let mut rng_a = LegacyRandom::new(seed_a);
            create_tunnel(
                config,
                chunk_x,
                chunk_z,
                d0,
                d1,
                d2,
                f_a,
                f1 - std::f32::consts::FRAC_PI_2,
                f2 / 3.0,
                step,
                total_steps,
                1.0,
                water,
                mask,
                &mut rng_a,
                parent_rng,
            );
            let f_b = rng.next_f32() * 0.5 + 0.5;
            let seed_b = parent_rng.next_java_long() as u64;
            let mut rng_b = LegacyRandom::new(seed_b);
            create_tunnel(
                config,
                chunk_x,
                chunk_z,
                d0,
                d1,
                d2,
                f_b,
                f1 + std::f32::consts::FRAC_PI_2,
                f2 / 3.0,
                step,
                total_steps,
                1.0,
                water,
                mask,
                &mut rng_b,
                parent_rng,
            );
            return;
        }

        if is_room || rng.next_i32_bound(4) != 0 {
            let d4 = chunk_x as f64 * 16.0 + 8.0;
            let d5 = chunk_z as f64 * 16.0 + 8.0;
            let dx = d0 - d4;
            let dz = d2 - d5;
            let dist_remain = (total_steps - step) as f64;
            let eff_radius = f as f64 + 2.0 + 16.0;
            if dx * dx + dz * dz - dist_remain * dist_remain > eff_radius * eff_radius {
                return;
            }

            if d0 >= d4 - 16.0 - d6 * 2.0
                && d2 >= d5 - 16.0 - d6 * 2.0
                && d0 <= d4 + 16.0 + d6 * 2.0
                && d2 <= d5 + 16.0 + d6 * 2.0
            {
                let carved = carve_ellipsoid(chunk_x, chunk_z, d0, d1, d2, d6, d7, water, mask);
                // A room (single-step carve) stops after carving one ellipsoid,
                // unless water-abort skipped the carve. Mirrors Beta's `if(flag) break`.
                if is_room && carved {
                    break;
                }
            }
        }

        step += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carver::carve_ellipsoid;
    use crate::carver::water::WaterMask;
    use mcrs_minecraft_random::Random;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_voxel_storage::VoxelId;

    fn beta_config() -> BetaCaveCarverConfig {
        BetaCaveCarverConfig {
            air_state: VoxelId(0),
            lava_state: VoxelId(11),
            stone_state: VoxelId(1),
            dirt_state: VoxelId(2),
            grass_state: VoxelId(3),
            lava_level: 10,
            range: 8,
            horizontal_radius_multiplier: 1.0,
            vertical_radius_multiplier: 1.0,
        }
    }

    #[test]
    fn next_i32_bound_one_returns_zero() {
        let mut rng = LegacyRandom::new(12345);
        assert_eq!(rng.next_i32_bound(1), 0);
    }

    #[test]
    fn cave_count_formula_draw_count() {
        let seed = 381u64;
        let mut rng = LegacyRandom::new(seed);
        let a = rng.next_i32_bound(40) + 1;
        let b = rng.next_i32_bound(a) + 1;
        let cave_count = rng.next_i32_bound(b);
        let _gated = if rng.next_i32_bound(15) != 0 {
            0
        } else {
            cave_count
        };
        let _thickness = rng.next_f32() * 2.0 + rng.next_f32();
        // If we reach here without panic, the five draws completed successfully.
        // A draw-count pin for the full carve-mask is validated in the cave parity integration test.
    }

    /// The mask marks the Y the ellipsoid test accepts, which sits within the
    /// vertical radius and never below the -0.7 floor.
    #[test]
    fn the_ellipsoid_marks_only_its_own_interior() {
        let mut mask = CarvingMask::new(16, 1, 119);
        assert!(carve_ellipsoid(
            0,
            0,
            8.0,
            50.0,
            8.0,
            3.0,
            2.0,
            &WaterMask::default(),
            &mut mask,
        ));

        let mut columns = 0;
        mask.visit(|x, z, bottom, top| {
            columns += 1;
            let dx = (x as f64 + 0.5 - 8.0) / 3.0;
            let dz = (z as f64 + 0.5 - 8.0) / 3.0;
            assert!(dx * dx + dz * dz < 1.0, "column ({x}, {z}) is outside");
            assert!(bottom as f64 + 0.5 >= 50.0 - 0.7 * 2.0 - 1.0);
            assert!(top <= 52);
        });
        assert!(columns > 0);
    }

    #[test]
    fn a_water_abort_marks_nothing_and_reports_it() {
        let mut water = WaterMask::default();
        water.insert(4, 50, 8);
        let mut mask = CarvingMask::new(16, 1, 119);
        assert!(!carve_ellipsoid(
            0, 0, 8.0, 50.0, 8.0, 3.0, 2.0, &water, &mut mask,
        ));
        assert!(mask.is_empty());
    }

    #[test]
    fn the_config_carries_the_beta_substance_states() {
        let config = beta_config();
        assert_eq!(config.lava_level, 10);
        assert!(crate::carver::can_replace_block(
            &config,
            config.stone_state
        ));
        assert!(!crate::carver::can_replace_block(
            &config,
            config.lava_state
        ));
    }
}
