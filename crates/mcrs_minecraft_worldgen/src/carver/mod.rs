pub mod cave;
pub mod config;

use crate::carver::config::BetaCaveCarverConfig;
use mcrs_protocol::BlockStateId;
use mcrs_random::Random;

pub trait WorldCarver {
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
        S: FnMut(i32, i32, i32, BlockStateId);
}

pub fn can_replace_block(config: &BetaCaveCarverConfig, state: BlockStateId) -> bool {
    state == config.stone_state || state == config.dirt_state || state == config.grass_state
}

pub fn get_carve_state(config: &BetaCaveCarverConfig, world_y: i32) -> BlockStateId {
    if world_y < config.lava_level {
        config.lava_state
    } else {
        config.air_state
    }
}

/// Returns false if carving should be aborted due to water adjacency.
///
/// Ports MapGenCaves water-abort scan (lines 98–113): outer Y-loop from j2+1 down
/// to i2-1, skipping interior via early-set `l3 = i2` — identical to the Java idiom.
pub fn water_abort_scan<G>(
    config: &BetaCaveCarverConfig,
    x_min: i32,
    x_max: i32,
    y_min: i32,
    y_max: i32,
    z_min: i32,
    z_max: i32,
    water_state: BlockStateId,
    stationary_water_state: BlockStateId,
    get_block: &G,
) -> bool
where
    G: Fn(i32, i32, i32) -> BlockStateId,
{
    let mut abort = false;
    let mut x = x_min;
    while !abort && x < x_max {
        let mut z = z_min;
        while !abort && z < z_max {
            let mut y = y_max + 1;
            while !abort && y >= y_min - 1 {
                if y >= 0 && y < 128 {
                    let state = get_block(x, y, z);
                    if state == water_state || state == stationary_water_state {
                        abort = true;
                    }
                    if y != y_min - 1 && x != x_min && x != x_max - 1 && z != z_min && z != z_max - 1 {
                        y = y_min;
                    }
                }
                y -= 1;
            }
            z += 1;
        }
        x += 1;
    }
    abort
}

/// Carve an ellipsoid at (d0, d1, d2) with horizontal radius d6 and vertical radius d7.
///
/// get_block/set_block receive chunk-local x (0..16), world y, chunk-local z (0..16).
/// Returns false if carving was aborted due to adjacent water.
pub fn carve_ellipsoid<G, S>(
    config: &BetaCaveCarverConfig,
    chunk_x: i32,
    chunk_z: i32,
    d0: f64,
    d1: f64,
    d2: f64,
    d6: f64,
    d7: f64,
    water_state: BlockStateId,
    stationary_water_state: BlockStateId,
    get_block: &G,
    set_block: &mut S,
) -> bool
where
    G: Fn(i32, i32, i32) -> BlockStateId,
    S: FnMut(i32, i32, i32, BlockStateId),
{
    let k1 = (d0 - d6).floor() as i32 - chunk_x * 16 - 1;
    let l1 = (d0 + d6).floor() as i32 - chunk_x * 16 + 1;
    let i2 = (d1 - d7).floor() as i32 - 1;
    let j2 = (d1 + d7).floor() as i32 + 1;
    let k2 = (d2 - d6).floor() as i32 - chunk_z * 16 - 1;
    let l2 = (d2 + d6).floor() as i32 - chunk_z * 16 + 1;

    let k1 = k1.max(0);
    let l1 = l1.min(16);
    let i2 = i2.max(1);
    let j2 = j2.min(120);
    let k2 = k2.max(0);
    let l2 = l2.min(16);

    if water_abort_scan(
        config,
        k1,
        l1,
        i2,
        j2,
        k2,
        l2,
        water_state,
        stationary_water_state,
        get_block,
    ) {
        return false;
    }

    for lx in k1..l1 {
        let d12 = (lx + chunk_x * 16) as f64 + 0.5 - d0;
        let d12 = d12 / d6;

        for lz in k2..l2 {
            let d13 = (lz + chunk_z * 16) as f64 + 0.5 - d2;
            let d13 = d13 / d6;
            if d12 * d12 + d13 * d13 >= 1.0 {
                continue;
            }

            let mut flag3 = false;
            let mut world_y = j2 - 1;
            while world_y >= i2 {
                let d14 = (world_y as f64 + 0.5 - d1) / d7;
                if d14 > -0.7 && d12 * d12 + d14 * d14 + d13 * d13 < 1.0 {
                    // Beta reads/writes the cell one block above the geometry-tested
                    // Y (Java's `i4 = j4 + 1`), while the lava threshold and the
                    // grass fixup remain relative to the tested Y.
                    let cell_y = world_y + 1;
                    let state = get_block(lx, cell_y, lz);
                    if state == config.grass_state {
                        flag3 = true;
                    }
                    if can_replace_block(config, state) {
                        if world_y < config.lava_level {
                            set_block(lx, cell_y, lz, config.lava_state);
                        } else {
                            set_block(lx, cell_y, lz, config.air_state);
                            if flag3 {
                                let below = get_block(lx, world_y, lz);
                                if below == config.dirt_state {
                                    set_block(lx, world_y, lz, config.grass_state);
                                }
                            }
                        }
                    }
                }
                world_y -= 1;
            }
        }
    }
    true
}
