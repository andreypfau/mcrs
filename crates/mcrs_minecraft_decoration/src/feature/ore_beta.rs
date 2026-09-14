use mcrs_minecraft_random::Random;
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::{BlocksMut, VoxelId};

#[derive(Clone, Debug)]
pub struct TargetBlockState {
    pub target: VoxelId,
    pub state: VoxelId,
}

#[derive(Clone, Debug)]
pub struct OreConfig {
    pub targets: Vec<TargetBlockState>,
    pub size: i32,
}

/// Port of WorldGenMinable.a — place a single Beta ore vein.
///
/// Beta overrides applied vs modern OreFeature:
///   - y-offset is `+2` (not modern `-2`)
///   - no blob overlap-cull pre-pass
///   - no air-exposure discard
///   - stone-only replacement via `targets` table
pub fn place_beta_ore<R: Random>(
    config: &OreConfig,
    origin: BlockPos,
    volume: &mut impl BlocksMut,
    rng: &mut R,
) {
    let (origin_x, origin_y, origin_z) = (origin.x, origin.y, origin.z);
    let size = config.size;

    let f = rng.next_f32() * std::f32::consts::PI;

    // Segment endpoints X
    let d0 = origin_x as f64 + 8.0 + (mcrs_voxel_math::mth::sin(f) * size as f32 / 8.0) as f64;
    let d1 = origin_x as f64 + 8.0 - (mcrs_voxel_math::mth::sin(f) * size as f32 / 8.0) as f64;
    // Segment endpoints Z
    let d2 = origin_z as f64 + 8.0 + (mcrs_voxel_math::mth::cos(f) * size as f32 / 8.0) as f64;
    let d3 = origin_z as f64 + 8.0 - (mcrs_voxel_math::mth::cos(f) * size as f32 / 8.0) as f64;
    // Segment endpoints Y — Beta WorldGenMinable uses `+2`, not modern OreFeature's `-2`
    let d4 = origin_y as f64 + rng.next_i32_bound(3) as f64 + 2.0;
    let d5 = origin_y as f64 + rng.next_i32_bound(3) as f64 + 2.0;

    for l in 0..=size {
        let d6 = d0 + (d1 - d0) * l as f64 / size as f64;
        let d7 = d4 + (d5 - d4) * l as f64 / size as f64;
        let d8 = d2 + (d3 - d2) * l as f64 / size as f64;

        // next_f64 matches Java nextDouble() draw count (two LCG advances).
        let d9 = rng.next_f64() * size as f64 / 16.0;
        let sin_step =
            (mcrs_voxel_math::mth::sin(l as f32 * std::f32::consts::PI / size as f32) + 1.0) as f64;
        let d10 = sin_step * d9 + 1.0;
        let d11 = sin_step * d9 + 1.0;

        let i1 = (d6 - d10 / 2.0).floor() as i32;
        let j1 = (d7 - d11 / 2.0).floor() as i32;
        let k1 = (d8 - d10 / 2.0).floor() as i32;
        let l1 = (d6 + d10 / 2.0).floor() as i32;
        let i2 = (d7 + d11 / 2.0).floor() as i32;
        let j2 = (d8 + d10 / 2.0).floor() as i32;

        for k2 in i1..=l1 {
            let d12 = (k2 as f64 + 0.5 - d6) / (d10 / 2.0);
            if d12 * d12 >= 1.0 {
                continue;
            }
            for l2 in j1..=i2 {
                let d13 = (l2 as f64 + 0.5 - d7) / (d11 / 2.0);
                if d12 * d12 + d13 * d13 >= 1.0 {
                    continue;
                }
                for i3 in k1..=j2 {
                    let d14 = (i3 as f64 + 0.5 - d8) / (d10 / 2.0);
                    if d12 * d12 + d13 * d13 + d14 * d14 < 1.0 {
                        let at = BlockPos::new(k2, l2, i3);
                        let current = volume.get(at);
                        if let Some(tgt) = config.targets.iter().find(|t| current == t.target) {
                            volume.set(at, tgt.state);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_voxel_storage::{Blocks, BoxVolume, VoxelId};

    const STONE: VoxelId = VoxelId(1);
    const COAL_ORE: VoxelId = VoxelId(16);
    const NON_STONE: VoxelId = VoxelId(2);

    fn beta_coal_config() -> OreConfig {
        OreConfig {
            targets: vec![TargetBlockState {
                target: STONE,
                state: COAL_ORE,
            }],
            size: 16,
        }
    }

    fn all_stone(width: i32, height: i32) -> BoxVolume {
        BoxVolume::filled(
            BlockPos::new(0, 0, 0),
            BlockPos::new(width - 1, height - 1, width - 1),
            STONE,
        )
    }

    #[test]
    fn place_only_replaces_stone_target() {
        let config = beta_coal_config();

        let mut world = all_stone(32, 128);

        // Mark a non-stone cell inside the likely vein range
        let non = BlockPos::new(12, 50, 12);
        world.set(non, NON_STONE);

        place_beta_ore(
            &config,
            BlockPos::new(0, 50, 0),
            &mut world,
            &mut LegacyRandom::new(12345),
        );

        assert!(
            world
                .iter()
                .all(|(_, state)| matches!(state, STONE | COAL_ORE | NON_STONE)),
            "ore placer must only write the target ore state"
        );
        assert!(world.iter().any(|(_, state)| state == COAL_ORE));
        assert_eq!(
            world.get(non),
            NON_STONE,
            "non-stone cell must not be replaced"
        );
    }

    /// Beta's `WorldGenMinable` offsets its segment endpoints by `+2` where the
    /// modern `OreFeature` uses `-2`, so a vein sits above the source position
    /// rather than straddling it.
    ///
    /// Read off the blocks rather than off the arithmetic: a test that
    /// recomputes the endpoints itself cannot fail when the placer changes.
    #[test]
    fn a_vein_sits_above_the_source_it_grew_from() {
        let config = beta_coal_config();
        let origin_y = 50;

        for seed in [12345u64, 381, 7, 99, 2024] {
            let mut world = all_stone(64, 128);
            place_beta_ore(
                &config,
                BlockPos::new(16, origin_y, 16),
                &mut world,
                &mut LegacyRandom::new(seed),
            );

            let lowest = world
                .iter()
                .filter(|(_, state)| *state == COAL_ORE)
                .map(|(at, _)| at.y)
                .min()
                .expect("the vein writes something");

            assert!(
                lowest > origin_y,
                "seed {seed}: the Beta +2 offset keeps every cell above {origin_y}, got {lowest}"
            );
        }
    }

    #[test]
    fn place_draw_count_anchor() {
        // Verify the per-vein draw count for size=8:
        //   1 next_f32 (angle) + 2 next_i32_bound (y endpoints) + (size+1) next_f64 (loop)
        //   = 3 + 9 = 12 method calls consuming 3 + 18 = 21 LCG advances.
        // This test pins the RNG state after placement for regression detection.
        let config = OreConfig {
            targets: vec![TargetBlockState {
                target: STONE,
                state: COAL_ORE,
            }],
            size: 8,
        };
        let mut world = all_stone(32, 128);

        let mut rng = LegacyRandom::new(12345);
        let state_before = rng.clone();

        place_beta_ore(&config, BlockPos::new(8, 40, 8), &mut world, &mut rng);

        // Replay the known draw sequence on the same starting state, then assert
        // the resulting RNG state matches what place() left behind.
        let mut replay = state_before;
        replay.next_f32(); // angle f
        replay.next_i32_bound(3); // d4
        replay.next_i32_bound(3); // d5
        for _ in 0..=config.size {
            // size+1 iterations
            replay.next_f64();
        }

        assert_eq!(
            rng, replay,
            "per-vein draw sequence must be exactly: 1 next_f32 + 2 next_i32_bound(3) + (size+1) next_f64"
        );
    }
}
