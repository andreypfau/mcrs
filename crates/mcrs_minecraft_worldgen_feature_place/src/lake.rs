use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen_feature::placer::{BiomeMask, Predicate, WorldGenVolume};

use crate::holds;
use crate::tree::provider::StateProvider;

const WIDTH: usize = 16;
const DEPTH: usize = 8;

#[derive(Clone, Debug)]
pub struct CompiledLake {
    pub fluid: StateProvider,
    pub barrier: StateProvider,
    pub can_place_feature: Predicate,
    pub can_replace_with_air_or_fluid: Predicate,
    pub can_replace_with_barrier: Predicate,
    pub ice: VoxelId,
    /// `Biome.shouldFreeze` reduced to the biome's own temperature,
    /// dropping the height and noise adjustment it applies above sea level and
    /// the block-light test that reads zero during generation. The ice pass
    /// draws nothing, so the seed chain is unaffected either way. Upgrade when
    /// the volume can answer a temperature at a position.
    pub freezing_biomes: BiomeMask,
}

fn index(x: usize, z: usize, y: usize) -> usize {
    (x * WIDTH + z) * DEPTH + y
}

/// A cell outside the blob whose six neighbours inside the grid include one
/// that is: the shell the barrier and the validation both walk.
fn is_liquid<W: WorldGenVolume>(volume: &W, state: VoxelId) -> bool {
    let world = volume.world();
    holds(&world.water_states, state) || holds(&world.lava_states, state)
}

fn on_shell(grid: &[bool], x: usize, z: usize, y: usize) -> bool {
    if grid[index(x, z, y)] {
        return false;
    }
    (x < 15 && grid[index(x + 1, z, y)])
        || (x > 0 && grid[index(x - 1, z, y)])
        || (z < 15 && grid[index(x, z + 1, y)])
        || (z > 0 && grid[index(x, z - 1, y)])
        || (y < 7 && grid[index(x, z, y + 1)])
        || (y > 0 && grid[index(x, z, y - 1)])
}

/// `LakeFeature.place`: four to seven ellipsoids carved into a 16×8×16 grid,
/// checked whole, then filled, walled and frozen.
pub fn place_lake<W: WorldGenVolume>(
    config: &CompiledLake,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
) -> bool {
    if origin.y <= volume.extent().min_y + 4 {
        return false;
    }
    let origin = origin + IVec3::new(-8, -4, -8);
    let grid = carve_blobs(rng);

    let fluid = config.fluid.state(volume, rng, origin);

    for x in 0..WIDTH {
        for z in 0..WIDTH {
            for y in 0..DEPTH {
                if !on_shell(&grid, x, z, y) {
                    continue;
                }
                let pos = origin + IVec3::new(x as i32, y as i32, z as i32);
                let state = volume.get(pos);
                // `BlockState.liquid`: the water and lava blocks themselves, not
                // everything that happens to hold their fluid.
                if y >= 4 && is_liquid(volume, state) {
                    return false;
                }
                if y < 4 && !holds(&volume.world().solid, state) && state != fluid {
                    return false;
                }
                if !config.can_place_feature.test(volume, pos) {
                    return false;
                }
            }
        }
    }

    for x in 0..WIDTH {
        for z in 0..WIDTH {
            for y in 0..DEPTH {
                if !grid[index(x, z, y)] {
                    continue;
                }
                let pos = origin + IVec3::new(x as i32, y as i32, z as i32);
                if config.can_replace_with_air_or_fluid.test(volume, pos) {
                    let state = if y >= 4 {
                        volume.world().cave_air
                    } else {
                        fluid
                    };
                    volume.set(pos, state);
                }
            }
        }
    }

    let barrier = config.barrier.state(volume, rng, origin);
    if !holds(&volume.world().air_states, barrier) {
        for x in 0..WIDTH {
            for z in 0..WIDTH {
                for y in 0..DEPTH {
                    if !on_shell(&grid, x, z, y) {
                        continue;
                    }
                    if y >= 4 && rng.next_i32_bound(2) == 0 {
                        continue;
                    }
                    let pos = origin + IVec3::new(x as i32, y as i32, z as i32);
                    if volume.holds(&volume.world().solid, pos)
                        && config.can_replace_with_barrier.test(volume, pos)
                    {
                        volume.set(pos, barrier);
                    }
                }
            }
        }
    }

    if holds(&volume.world().water_states, fluid) {
        for x in 0..WIDTH {
            for z in 0..WIDTH {
                let pos = origin + IVec3::new(x as i32, 4, z as i32);
                let freezes = config.freezing_biomes.contains(volume.biome(pos) as usize)
                    && volume.holds(&volume.world().water_states, pos);
                if freezes && config.can_replace_with_air_or_fluid.test(volume, pos) {
                    volume.set(pos, config.ice);
                }
            }
        }
    }
    true
}

/// Four to seven ellipsoids unioned into the 16×8×16 grid, each drawn as three
/// sizes then three centres.
fn carve_blobs(rng: &mut XoroshiroRandom) -> Vec<bool> {
    let mut grid = vec![false; WIDTH * WIDTH * DEPTH];
    let spots = rng.next_i32_bound(4) + 4;
    for _ in 0..spots {
        let x_size = rng.next_f64() * 6.0 + 3.0;
        let y_size = rng.next_f64() * 4.0 + 2.0;
        let z_size = rng.next_f64() * 6.0 + 3.0;
        let x_center = rng.next_f64() * (16.0 - x_size - 2.0) + 1.0 + x_size / 2.0;
        let y_center = rng.next_f64() * (8.0 - y_size - 4.0) + 2.0 + y_size / 2.0;
        let z_center = rng.next_f64() * (16.0 - z_size - 2.0) + 1.0 + z_size / 2.0;
        for x in 1..15 {
            for z in 1..15 {
                for y in 1..7 {
                    let dx = (x as f64 - x_center) / (x_size / 2.0);
                    let dy = (y as f64 - y_center) / (y_size / 2.0);
                    let dz = (z as f64 - z_center) / (z_size / 2.0);
                    if dx * dx + dy * dy + dz * dz < 1.0 {
                        grid[index(x, z, y)] = true;
                    }
                }
            }
        }
    }
    grid
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_chunk::Blocks;
    use mcrs_minecraft_worldgen_feature::placer::mask_of;
    use std::sync::Arc;

    use fixedbitset::FixedBitSet;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const LAVA: VoxelId = VoxelId(2);
    const WATER: VoxelId = VoxelId(3);
    const ICE: VoxelId = VoxelId(4);
    const CAVE_AIR: VoxelId = VoxelId(5);

    fn config(fluid: VoxelId) -> CompiledLake {
        CompiledLake {
            fluid: StateProvider::Simple(fluid),
            barrier: StateProvider::Simple(STONE),
            can_place_feature: Predicate::True,
            can_replace_with_air_or_fluid: Predicate::True,
            can_replace_with_barrier: Predicate::True,
            ice: ICE,
            freezing_biomes: Arc::new(FixedBitSet::from_iter([0])),
        }
    }

    const ORIGIN: BlockPos = BlockPos::new(0, 40, 0);

    fn solid_rock() -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.cave_air = CAVE_AIR;
        volume.world.water_states = mask_of([WATER]);
        volume.world.lava_states = mask_of([LAVA]);
        volume.world.solid = mask_of([STONE]);
        for x in -16..=16 {
            for z in -16..=16 {
                for y in 20..=60 {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        volume
    }

    #[test]
    fn a_lake_too_close_to_the_bottom_draws_nothing() {
        let mut volume = solid_rock();
        let mut rng = XoroshiroRandom::new(1);
        let before = rng.clone();
        assert!(!place_lake(
            &config(LAVA),
            &mut volume,
            &mut rng,
            BlockPos::new(0, -60, 0)
        ));
        assert_eq!(rng, before);
    }

    /// One spot count, six doubles per spot, then one draw per shell cell
    /// above the water line that the barrier pass reaches.
    #[test]
    fn lake_draw_count_anchor() {
        let mut volume = solid_rock();
        let mut rng = XoroshiroRandom::new(0x1a4e);
        let mut replay = rng.clone();
        assert!(place_lake(&config(LAVA), &mut volume, &mut rng, ORIGIN));

        let spots = replay.next_i32_bound(4) + 4;
        for _ in 0..spots * 6 {
            replay.next_f64();
        }
        let upper_shell = upper_shell_cells(&mut XoroshiroRandom::new(0x1a4e));
        for _ in 0..upper_shell {
            replay.next_i32_bound(2);
        }
        assert_eq!(rng, replay, "lake draw sequence");
    }

    /// The shell cells at or above the water line, which is where the barrier
    /// pass spends its one draw each.
    fn upper_shell_cells(rng: &mut XoroshiroRandom) -> usize {
        let grid = carve_blobs(rng);
        let mut count = 0;
        for x in 0..WIDTH {
            for z in 0..WIDTH {
                for y in 4..DEPTH {
                    if on_shell(&grid, x, z, y) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    #[test]
    fn the_blob_is_fluid_below_the_line_and_air_above() {
        let mut volume = solid_rock();
        let mut rng = XoroshiroRandom::new(0x1a4e);
        assert!(place_lake(&config(LAVA), &mut volume, &mut rng, ORIGIN));
        let below = volume
            .writes
            .iter()
            .any(|((_, y, _), state)| *state == LAVA && *y < ORIGIN.y);
        let above = volume
            .writes
            .iter()
            .any(|((_, y, _), state)| *state == CAVE_AIR && *y >= ORIGIN.y);
        assert!(below, "the bottom half is fluid");
        assert!(above, "the top half is cave air");
    }

    /// A shell cell that already holds a liquid above the water line rejects
    /// the whole lake — after the fluid provider has been asked for its state.
    #[test]
    fn a_liquid_on_the_upper_shell_rejects_the_lake() {
        let mut volume = solid_rock();
        for x in -16..=16 {
            for z in -16..=16 {
                for y in ORIGIN.y..=ORIGIN.y + 4 {
                    volume.blocks.insert((x, y, z), WATER);
                }
            }
        }
        let mut rng = XoroshiroRandom::new(0x1a4e);
        assert!(!place_lake(&config(LAVA), &mut volume, &mut rng, ORIGIN));
        assert!(volume.writes.is_empty());
    }

    /// The freeze pass runs over the layer one above the fluid, which the blob
    /// itself leaves as cave air. Only a cell the blob never reached — a corner
    /// of the grid, outside even the shell — keeps its water and can freeze.
    #[test]
    fn only_water_the_blob_never_reached_freezes() {
        let mut volume = solid_rock();
        let corner = BlockPos::new(ORIGIN.x - 8, ORIGIN.y, ORIGIN.z - 8);
        volume.blocks.insert((corner.x, corner.y, corner.z), WATER);
        let mut rng = XoroshiroRandom::new(0x1a4e);
        assert!(place_lake(&config(WATER), &mut volume, &mut rng, ORIGIN));
        assert_eq!(volume.get(corner), ICE);
    }

    #[test]
    fn a_lava_lake_never_freezes() {
        let mut volume = solid_rock();
        let corner = BlockPos::new(ORIGIN.x - 8, ORIGIN.y, ORIGIN.z - 8);
        volume.blocks.insert((corner.x, corner.y, corner.z), WATER);
        let mut rng = XoroshiroRandom::new(0x1a4e);
        assert!(place_lake(&config(LAVA), &mut volume, &mut rng, ORIGIN));
        assert!(!volume.writes.iter().any(|(_, state)| *state == ICE));
    }
}
