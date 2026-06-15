use crate::carver::config::BetaCaveCarverConfig;
use crate::carver::{carve_ellipsoid, WorldCarver};
use mcrs_protocol::BlockStateId;
use mcrs_random::legacy::LegacyRandom;
use mcrs_random::Random;

/// Java beta `MathHelper.sin(x)`: lookup-table approximation matching the 65536-entry table
/// built at class-load time via `(float)Math.sin(i * PI * 2.0 / 65536.0)`.
///
/// Using accurate `f32::sin` instead shifts ellipsoid radii at boundary cases,
/// causing single-block carve/no-carve divergence from the Java reference.
fn math_helper_sin(x: f32) -> f32 {
    let idx = ((x * 10430.378_f32) as i32 as u32) & 0xFFFF;
    f64::sin(idx as f64 * (std::f64::consts::TAU / 65536.0)) as f32
}

/// Java beta `MathHelper.cos(x)`: same table as `math_helper_sin`, offset by π/2.
///
/// Java: `SIN_TABLE[(int)(x * 10430.378F + 16384.0F) & 65535]`
fn math_helper_cos(x: f32) -> f32 {
    let idx = ((x * 10430.378_f32 + 16384.0_f32) as i32 as u32) & 0xFFFF;
    f64::sin(idx as f64 * (std::f64::consts::TAU / 65536.0)) as f32
}

pub struct CaveWorldCarver;

impl WorldCarver for CaveWorldCarver {
    fn carve<R: Random, G, S>(
        &self,
        config: &BetaCaveCarverConfig,
        chunk_x: i32,
        chunk_z: i32,
        origin_x: i32,
        origin_z: i32,
        get_block: G,
        set_block: S,
        rng: &mut R,
    ) where
        G: Fn(i32, i32, i32) -> BlockStateId,
        S: FnMut(i32, i32, i32, BlockStateId),
    {
        let mut set_block = set_block;
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
                    &get_block,
                    &mut set_block,
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
                    &get_block,
                    &mut set_block,
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
fn create_tunnel<R: Random, G, S>(
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
    get_block: &G,
    set_block: &mut S,
    rng: &mut LegacyRandom,
    parent_rng: &mut R,
) where
    R: Random,
    G: Fn(i32, i32, i32) -> BlockStateId,
    S: FnMut(i32, i32, i32, BlockStateId),
{
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
                get_block,
                set_block,
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
                get_block,
                set_block,
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
                let carved = carve_ellipsoid(
                    config,
                    chunk_x,
                    chunk_z,
                    d0,
                    d1,
                    d2,
                    d6,
                    d7,
                    config.water_state,
                    config.stationary_water_state,
                    get_block,
                    set_block,
                );
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
    use crate::carver::config::BetaCaveCarverConfig;
    use mcrs_random::legacy::LegacyRandom;
    use mcrs_random::Random;
    use std::cell::Cell;

    fn beta_config() -> BetaCaveCarverConfig {
        BetaCaveCarverConfig {
            air_state: BlockStateId(0),
            lava_state: BlockStateId(11),
            stone_state: BlockStateId(1),
            dirt_state: BlockStateId(2),
            grass_state: BlockStateId(3),
            water_state: BlockStateId(0),
            stationary_water_state: BlockStateId(0),
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

    #[test]
    fn carving_writes_correct_states_by_y_level() {
        let config = beta_config();
        let air = config.air_state;
        let lava = config.lava_state;
        let stone = config.stone_state;

        const WIDTH: usize = 16;
        const HEIGHT: usize = 128;

        let blocks: Vec<Cell<BlockStateId>> =
            (0..WIDTH * WIDTH * HEIGHT).map(|_| Cell::new(stone)).collect();

        let idx = |lx: i32, wy: i32, lz: i32| -> usize {
            (lx as usize * WIDTH + lz as usize) * HEIGHT + wy as usize
        };

        let get_block = |lx: i32, wy: i32, lz: i32| -> BlockStateId {
            if wy < 0 || wy >= HEIGHT as i32 || lx < 0 || lx >= WIDTH as i32 || lz < 0 || lz >= WIDTH as i32 {
                return BlockStateId(0);
            }
            blocks[idx(lx, wy, lz)].get()
        };

        let mut carved_above: Vec<(i32, i32, i32, BlockStateId)> = Vec::new();
        carve_ellipsoid(
            &config,
            0,
            0,
            8.0,
            15.0,
            8.0,
            3.0,
            2.0,
            BlockStateId(0),
            BlockStateId(0),
            &get_block,
            &mut |lx, wy, lz, state| {
                carved_above.push((lx, wy, lz, state));
                blocks[idx(lx, wy, lz)].set(state);
            },
        );

        for (_, wy, _, state) in &carved_above {
            if *wy >= config.lava_level {
                assert_eq!(*state, air, "Y={} should be air", wy);
            } else {
                assert_eq!(*state, lava, "Y={} should be lava", wy);
            }
        }

        let blocks2: Vec<Cell<BlockStateId>> =
            (0..WIDTH * WIDTH * HEIGHT).map(|_| Cell::new(stone)).collect();
        let get_block2 = |lx: i32, wy: i32, lz: i32| -> BlockStateId {
            if wy < 0 || wy >= HEIGHT as i32 || lx < 0 || lx >= WIDTH as i32 || lz < 0 || lz >= WIDTH as i32 {
                return BlockStateId(0);
            }
            blocks2[idx(lx, wy, lz)].get()
        };
        carve_ellipsoid(
            &config,
            0,
            0,
            8.0,
            5.0,
            8.0,
            3.0,
            2.0,
            BlockStateId(0),
            BlockStateId(0),
            &get_block2,
            &mut |lx, wy, lz, state| {
                if wy < config.lava_level {
                    assert_eq!(state, lava, "Y={} below lava_level must be lava", wy);
                } else {
                    assert_eq!(state, air, "Y={} above lava_level must be air", wy);
                }
                blocks2[idx(lx, wy, lz)].set(state);
            },
        );
    }
}
