use crate::jmath::{clampf, jmax, sqrt};
use crate::noise::simplex::SimplexNoise;
use crate::volume::Volume;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

const ISLAND_THRESHOLD: f32 = -0.9;

/// `EndIslandFunction.range()`.
pub fn range() -> crate::interval::Interval {
    crate::interval::Interval::of(-0.84375, 0.5625)
}

#[derive(Clone, Debug)]
pub struct EndIslandParams {
    noise: SimplexNoise,
}

impl EndIslandParams {
    /// The raw world seed, not a per-function hashed fork. The 17292 discarded
    /// draws and the three origin draws the at-origin constructor still makes
    /// are both stream positions the permutation table depends on.
    pub fn new(world_seed: u64) -> Self {
        let mut random = LegacyRandom::new(world_seed);
        for _ in 0..17292 {
            random.next_i32();
        }
        Self {
            noise: SimplexNoise::from_random_at_origin(&mut random),
        }
    }

    pub fn eval(&self, out: &mut [f32], ext: &Volume) {
        crate::kernel::each_column(out, ext, |run, ix, iz| {
            let bx = ext.block_x(ix as i32);
            let bz = ext.block_z(iz as i32);
            run.fill((self.height(bx / 8, bz / 8) - 8.0) / 128.0);
        });
    }

    fn height(&self, section_x: i32, section_z: i32) -> f32 {
        let chunk_x = section_x / 2;
        let chunk_z = section_z / 2;
        let sub_section_x = section_x % 2;
        let sub_section_z = section_z % 2;
        let mut height = -100.0f32;
        for offset_x in -12..=12 {
            for offset_z in -12..=12 {
                let cell_x = (chunk_x + offset_x) as i64;
                let cell_z = (chunk_z + offset_z) as i64;
                if cell_x * cell_x + cell_z * cell_z <= 4096 {
                    continue;
                }
                // `SimplexNoise.get` returns a float, so the value narrows before
                // the threshold test rather than being compared as a double.
                let sample = self.noise.sample_2d(cell_x as f64, cell_z as f64, 1.0, 1.0) as f32;
                if sample >= ISLAND_THRESHOLD {
                    continue;
                }
                let island_size =
                    ((cell_x as f32).abs() * 3439.0 + (cell_z as f32).abs() * 147.0) % 13.0 + 9.0;
                let dx = (sub_section_x - offset_x * 2) as f32;
                let dz = (sub_section_z - offset_z * 2) as f32;
                let candidate = 100.0 - sqrt(dx * dx + dz * dz) * island_size;
                height = jmax(height, clampf(candidate, -100.0, 80.0));
            }
        }
        height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::IVec3;

    fn value_at(params: &EndIslandParams, bx: i32, bz: i32) -> f32 {
        let mut out = [0.0f32];
        params.eval(&mut out, &Volume::point(IVec3::new(bx, 0, bz)));
        out[0]
    }

    #[test]
    fn matches_the_vanilla_oracle() {
        const EXPECTED: &[(u64, i32, i32, u32)] = &[
            (0, 0, 0, 0xbf58_0000),
            (0, 1000, 1000, 0x3e24_e8e8),
            (0, -1234, 5678, 0x3f10_0000),
            (42, 2000, -40, 0xbf40_9450),
            (42, 50000, 50000, 0xbdf7_af08),
            (845, -1234, 5678, 0xbd4f_a270),
        ];
        for &(seed, bx, bz, expected) in EXPECTED {
            let params = EndIslandParams::new(seed);
            let actual = value_at(&params, bx, bz);
            assert_eq!(
                actual.to_bits(),
                expected,
                "seed {seed} at ({bx}, {bz}): got {actual}"
            );
        }
    }

    #[test]
    fn the_section_division_truncates_towards_zero() {
        let params = EndIslandParams::new(42);
        let at = |bz| value_at(&params, -4000, bz);
        assert_eq!(at(-1).to_bits(), 0xbe01_5510);
        assert_eq!(at(1), at(-1), "-1 and 1 are both section 0");
        // What floor division would have answered for z = -1.
        assert_eq!(at(-8).to_bits(), 0xbe67_7890);
    }

    #[test]
    fn the_extent_holds_one_value_per_column_in_layout_order() {
        let params = EndIslandParams::new(42);
        let ext = Volume::new(
            IVec3::new(2, 1, 2),
            IVec3::new(-4000, 0, -13),
            IVec3::new(64, 1, 64),
        );
        let mut out = vec![0.0f32; 4];
        params.eval(&mut out, &ext);
        for (k, (ix, iz)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
            assert_eq!(out[k], value_at(&params, -4000 + 64 * ix, -13 + 64 * iz));
        }
    }

    #[test]
    fn the_value_ignores_y() {
        let params = EndIslandParams::new(42);
        let mut low = [0.0f32];
        let mut high = [0.0f32];
        params.eval(&mut low, &Volume::point(IVec3::new(-4000, -64, -13)));
        params.eval(&mut high, &Volume::point(IVec3::new(-4000, 320, -13)));
        assert_eq!(low, high);
        assert_ne!(
            value_at(&params, -4000, -13),
            value_at(&params, 50000, 50000)
        );
    }
}
