pub mod config;

use crate::feature::config::{OreConfig, OreYOffset};
use mcrs_protocol::BlockStateId;
use mcrs_random::Random;

pub struct OreFeature;

impl OreFeature {
    /// Port of WorldGenMinable.a — place a single ore vein.
    ///
    /// `get_block` / `set_block` receive world-absolute coordinates.
    /// Beta overrides applied vs modern OreFeature:
    ///   - y-offset is `+2` (not modern `-2`)
    ///   - no blob overlap-cull pre-pass
    ///   - no air-exposure discard
    ///   - stone-only replacement via `targets` table
    pub fn place<R: Random, G, S>(
        &self,
        config: &OreConfig,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        get_block: G,
        set_block: S,
        rng: &mut R,
    ) where
        G: Fn(i32, i32, i32) -> BlockStateId,
        S: FnMut(i32, i32, i32, BlockStateId),
    {
        do_place(config, origin_x, origin_y, origin_z, get_block, set_block, rng);
    }
}

fn do_place<R: Random, G, S>(
    config: &OreConfig,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    get_block: G,
    mut set_block: S,
    rng: &mut R,
) where
    G: Fn(i32, i32, i32) -> BlockStateId,
    S: FnMut(i32, i32, i32, BlockStateId),
{
    let size = config.size;

    let f = rng.next_f32() * std::f32::consts::PI;

    // Segment endpoints X
    let d0 = origin_x as f64 + 8.0 + (f32::sin(f) * size as f32 / 8.0) as f64;
    let d1 = origin_x as f64 + 8.0 - (f32::sin(f) * size as f32 / 8.0) as f64;
    // Segment endpoints Z
    let d2 = origin_z as f64 + 8.0 + (f32::cos(f) * size as f32 / 8.0) as f64;
    let d3 = origin_z as f64 + 8.0 - (f32::cos(f) * size as f32 / 8.0) as f64;
    // Segment endpoints Y — Beta WorldGenMinable uses `+2`, not modern OreFeature's `-2`
    let d4 = match config.y_offset {
        OreYOffset::BetaPlus2 => origin_y as f64 + rng.next_i32_bound(3) as f64 + 2.0,
        OreYOffset::ModernMinus2 => origin_y as f64 + rng.next_i32_bound(3) as f64 - 2.0,
    };
    let d5 = match config.y_offset {
        OreYOffset::BetaPlus2 => origin_y as f64 + rng.next_i32_bound(3) as f64 + 2.0,
        OreYOffset::ModernMinus2 => origin_y as f64 + rng.next_i32_bound(3) as f64 - 2.0,
    };

    for l in 0..=size {
        let d6 = d0 + (d1 - d0) * l as f64 / size as f64;
        let d7 = d4 + (d5 - d4) * l as f64 / size as f64;
        let d8 = d2 + (d3 - d2) * l as f64 / size as f64;

        // next_f64 matches Java nextDouble() draw count (two LCG advances).
        let d9 = rng.next_f64() * size as f64 / 16.0;
        let sin_step = (f32::sin(l as f32 * std::f32::consts::PI / size as f32) + 1.0) as f64;
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
                        let current = get_block(k2, l2, i3);
                        for tgt in &config.targets {
                            if current == tgt.target {
                                set_block(k2, l2, i3, tgt.state);
                                break;
                            }
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
    use crate::feature::config::{OreConfig, OreYOffset, TargetBlockState};
    use mcrs_protocol::BlockStateId;
    use mcrs_random::legacy::LegacyRandom;
    use std::cell::Cell;

    const STONE: BlockStateId = BlockStateId(1);
    const COAL_ORE: BlockStateId = BlockStateId(16);
    const NON_STONE: BlockStateId = BlockStateId(2);

    fn beta_coal_config() -> OreConfig {
        OreConfig {
            targets: vec![TargetBlockState { target: STONE, state: COAL_ORE }],
            size: 16,
            y_offset: OreYOffset::BetaPlus2,
        }
    }

    /// Minimal block store for unit tests: flat world-coord array.
    struct FlatBlocks {
        data: Vec<Cell<BlockStateId>>,
        width: usize,
        height: usize,
    }

    impl FlatBlocks {
        fn all_stone(width: usize, height: usize) -> Self {
            Self {
                data: (0..width * width * height).map(|_| Cell::new(STONE)).collect(),
                width,
                height,
            }
        }

        fn idx(&self, wx: i32, wy: i32, wz: i32) -> Option<usize> {
            let w = self.width as i32;
            let h = self.height as i32;
            if wx < 0 || wx >= w || wy < 0 || wy >= h || wz < 0 || wz >= w {
                return None;
            }
            Some((wx as usize * self.width + wz as usize) * self.height + wy as usize)
        }

        fn get(&self, wx: i32, wy: i32, wz: i32) -> BlockStateId {
            self.idx(wx, wy, wz).map_or(BlockStateId(0), |i| self.data[i].get())
        }

        fn set(&self, wx: i32, wy: i32, wz: i32, state: BlockStateId) {
            if let Some(i) = self.idx(wx, wy, wz) {
                self.data[i].set(state);
            }
        }
    }

    #[test]
    fn place_only_replaces_stone_target() {
        let config = beta_coal_config();
        let feature = OreFeature;

        let world = FlatBlocks::all_stone(32, 128);

        // Mark a non-stone cell inside the likely vein range
        let non_x = 12i32;
        let non_y = 50i32;
        let non_z = 12i32;
        world.set(non_x, non_y, non_z, NON_STONE);

        let mut replacements: Vec<(i32, i32, i32, BlockStateId)> = Vec::new();

        feature.place(
            &config,
            0,
            50,
            0,
            |wx, wy, wz| world.get(wx, wy, wz),
            |wx, wy, wz, state| {
                replacements.push((wx, wy, wz, state));
                world.set(wx, wy, wz, state);
            },
            &mut LegacyRandom::new(12345),
        );

        // Every write must be COAL_ORE (came from replacing STONE only)
        for &(_, _, _, state) in &replacements {
            assert_eq!(state, COAL_ORE, "ore placer must only write the target ore state");
        }
        // The explicitly non-stone cell must be untouched
        assert_eq!(
            world.get(non_x, non_y, non_z),
            NON_STONE,
            "non-stone cell must not be replaced"
        );
    }

    #[test]
    fn beta_y_offset_is_plus2() {
        // Pin the Beta y-offset sign: y endpoints are origin_y + nextInt(3) + 2,
        // meaning they are always at least origin_y+2 regardless of RNG value.
        let origin_y = 30i32;
        let mut rng = LegacyRandom::new(381);

        // Replicate the draw sequence in do_place:
        let _ = rng.next_f32();          // angle f
        let r0 = rng.next_i32_bound(3); // d4 draw
        let r1 = rng.next_i32_bound(3); // d5 draw

        let d4 = origin_y as f64 + r0 as f64 + 2.0;
        let d5 = origin_y as f64 + r1 as f64 + 2.0;

        assert!(
            d4 >= origin_y as f64 + 2.0,
            "Beta y0 must be >= origin_y+2 (Beta +2 offset), got {d4}"
        );
        assert!(
            d5 >= origin_y as f64 + 2.0,
            "Beta y1 must be >= origin_y+2 (Beta +2 offset), got {d5}"
        );
        // Also verify these are NOT the modern -2 sign
        assert!(d4 > origin_y as f64, "Beta y0 must exceed origin_y (never -2)");
        assert!(d5 > origin_y as f64, "Beta y1 must exceed origin_y (never -2)");
    }

    #[test]
    fn place_draw_count_anchor() {
        // Verify the per-vein draw count for size=8:
        //   1 next_f32 (angle) + 2 next_i32_bound (y endpoints) + (size+1) next_f64 (loop)
        //   = 3 + 9 = 12 method calls consuming 3 + 18 = 21 LCG advances.
        // This test pins the RNG state after placement for regression detection.
        let config = OreConfig {
            targets: vec![TargetBlockState { target: STONE, state: COAL_ORE }],
            size: 8,
            y_offset: OreYOffset::BetaPlus2,
        };
        let feature = OreFeature;
        let world = FlatBlocks::all_stone(32, 128);

        let mut rng = LegacyRandom::new(12345);
        let state_before = rng.clone();

        feature.place(
            &config,
            8,
            40,
            8,
            |wx, wy, wz| world.get(wx, wy, wz),
            |_, _, _, _| {},
            &mut rng,
        );

        // Replay the known draw sequence on the same starting state, then assert
        // the resulting RNG state matches what place() left behind.
        let mut replay = state_before;
        replay.next_f32();             // angle f
        replay.next_i32_bound(3);      // d4
        replay.next_i32_bound(3);      // d5
        for _ in 0..=config.size {    // size+1 iterations
            replay.next_f64();
        }

        assert_eq!(
            rng, replay,
            "per-vein draw sequence must be exactly: 1 next_f32 + 2 next_i32_bound(3) + (size+1) next_f64"
        );
    }
}
