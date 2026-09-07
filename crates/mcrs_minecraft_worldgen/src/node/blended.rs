use crate::jmath;
use crate::kernel::each_column;
use crate::noise::perlin::SmearedPerlinNoise;
use crate::noise::stack::{ColumnScratch, NoiseStack};
use crate::volume::Volume;
use mcrs_minecraft_random::RandomSource;
use std::fmt;
use std::sync::Arc;

pub const NOISE_SEED: &str = "minecraft:terrain";

const BASE_SCALE: f64 = 684.412;
/// Java writes the float literal `0.99998474F` into a `double` field, so the
/// widened value is exactly `(2^16 - 1) / 2^16` and every limit amplitude lands
/// on an exact power of two.
const LIMIT_FACTOR: f64 = 0.99998474f32 as f64;
const MAIN_FACTOR: f64 = 12.75;
const LIMIT_FIRST_OCTAVE: i32 = -15;
const MAIN_FIRST_OCTAVE: i32 = -7;

struct Inner {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
    /// 1.0 for 26.3, whose layer value factors already carry the scaling. Beta
    /// omits the trailing division by 128 that those factors fold in, so its
    /// terrain is exactly 128 times larger. A power of two, so this is exact.
    final_scale: f32,
    xz_multiplier: f64,
    y_multiplier: f64,
    main_xz_multiplier: f64,
    main_y_multiplier: f64,
    min_limit: NoiseStack<SmearedPerlinNoise>,
    max_limit: NoiseStack<SmearedPerlinNoise>,
    main: NoiseStack<SmearedPerlinNoise>,
}

/// Vanilla's `BlendedNoise.createFbm`: octaves highest frequency first, which is
/// the order they draw from the stream, each carrying its own smear scale.
fn create_fbm(
    random: &mut RandomSource,
    first_octave: i32,
    smear_scale_y: f64,
    value_factor: f64,
) -> NoiseStack<SmearedPerlinNoise> {
    let octaves = -first_octave + 1;
    let mut frequency = 1.0f64;
    let mut value_factor = value_factor / (2.0f64.powi(octaves) - 1.0);
    let mut stack = NoiseStack::builder();
    for _ in 0..octaves {
        stack.add(
            SmearedPerlinNoise::from_random(random, smear_scale_y * frequency),
            frequency,
            value_factor as f32,
        );
        frequency *= 0.5;
        value_factor *= 2.0;
    }
    stack.build()
}

#[derive(Clone)]
pub struct BlendedParams {
    inner: Arc<Inner>,
}

impl BlendedParams {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        random: &mut RandomSource,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
        final_scale: f32,
    ) -> Self {
        let xz_multiplier = BASE_SCALE * xz_scale;
        let y_multiplier = BASE_SCALE * y_scale;
        let limit_smear_scale_y = y_multiplier * smear_scale_multiplier;
        let main_smear_scale_y = limit_smear_scale_y / y_factor;

        // The three stacks share one stream in this order; the `let` bindings are
        // what fixes it, so do not fold them into the struct literal.
        let min_limit = create_fbm(random, LIMIT_FIRST_OCTAVE, limit_smear_scale_y, LIMIT_FACTOR);
        let max_limit = create_fbm(random, LIMIT_FIRST_OCTAVE, limit_smear_scale_y, LIMIT_FACTOR);
        let main = create_fbm(random, MAIN_FIRST_OCTAVE, main_smear_scale_y, MAIN_FACTOR);

        Self {
            inner: Arc::new(Inner {
                xz_scale,
                y_scale,
                xz_factor,
                y_factor,
                smear_scale_multiplier,
                final_scale,
                xz_multiplier,
                y_multiplier,
                main_xz_multiplier: xz_multiplier / xz_factor,
                main_y_multiplier: y_multiplier / y_factor,
                min_limit,
                max_limit,
                main,
            }),
        }
    }

    pub fn eval(&self, out: &mut [f32], ext: &Volume) {
        let n = ext.size().y as usize;
        let mut floats = vec![0.0f32; 2 * n];
        let mut scratch = ColumnScratch::default();
        let inner = &*self.inner;

        each_column(out, ext, |run, ix, iz| {
            let (min, max) = floats.split_at_mut(n);
            let bx = ext.block_x(ix as i32);
            let bz = ext.block_z(iz as i32);

            inner.main.fill_column_at(
                run,
                bx,
                bz,
                ext,
                inner.main_xz_multiplier,
                inner.main_y_multiplier,
                &mut scratch,
            );
            for alpha in run.iter_mut() {
                *alpha = jmath::clampf(*alpha + 0.5, 0.0, 1.0);
            }
            // An alpha pinned to an endpoint returns one limit untouched, so the
            // other stack is never read and its sixteen octaves need not be
            // sampled. The test is per column, which is where it pays.
            if run.iter().any(|&alpha| alpha != 1.0) {
                inner.min_limit.fill_column_at(
                    min,
                    bx,
                    bz,
                    ext,
                    inner.xz_multiplier,
                    inner.y_multiplier,
                    &mut scratch,
                );
            }
            if run.iter().any(|&alpha| alpha != 0.0) {
                inner.max_limit.fill_column_at(
                    max,
                    bx,
                    bz,
                    ext,
                    inner.xz_multiplier,
                    inner.y_multiplier,
                    &mut scratch,
                );
            }

            for (i, slot) in run.iter_mut().enumerate() {
                let alpha = *slot;
                *slot = if alpha == 0.0 {
                    min[i]
                } else if alpha == 1.0 {
                    max[i]
                } else {
                    jmath::lerp(alpha, min[i], max[i])
                };
            }
            if inner.final_scale != 1.0 {
                for slot in run.iter_mut() {
                    *slot *= inner.final_scale;
                }
            }
        });
    }
}

/// Hand-written because the derived form would print forty octaves of
/// 256-byte permutation table.
impl fmt::Debug for BlendedParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlendedParams")
            .field("xz_scale", &self.inner.xz_scale)
            .field("y_scale", &self.inner.y_scale)
            .field("xz_factor", &self.inner.xz_factor)
            .field("y_factor", &self.inner.y_factor)
            .field("smear_scale_multiplier", &self.inner.smear_scale_multiplier)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::IVec3;

    fn column() -> Volume {
        Volume::new(
            IVec3::new(1, 6, 1),
            IVec3::new(7, -32, -13),
            IVec3::new(1, 4, 1),
        )
    }

    fn overworld() -> BlendedParams {
        let mut random = RandomSource::new(42, false);
        BlendedParams::new(&mut random, 0.25, 0.125, 80.0, 160.0, 8.0, 1.0)
    }

    #[test]
    fn the_beta_scale_is_an_exact_factor_of_128() {
        let mut a = RandomSource::new(42, false);
        let modern = BlendedParams::new(&mut a, 0.25, 0.125, 80.0, 160.0, 8.0, 1.0);
        let mut b = RandomSource::new(42, false);
        let beta = BlendedParams::new(&mut b, 0.25, 0.125, 80.0, 160.0, 8.0, 128.0);

        let mut m = [0.0f32; 6];
        let mut t = [0.0f32; 6];
        modern.eval(&mut m, &column());
        beta.eval(&mut t, &column());
        for (i, (&x, &y)) in m.iter().zip(&t).enumerate() {
            assert_eq!(y, x * 128.0, "element {i}");
            assert_eq!(y / 128.0, x, "128 is a power of two, so this round-trips");
        }
        assert!(
            m.iter().any(|&v| v != 0.0),
            "the sample must not be trivial"
        );
    }

    #[test]
    fn a_point_query_at_each_height_reproduces_the_run() {
        let params = overworld();
        let ext = column();
        let mut run = [0.0f32; 6];
        params.eval(&mut run, &ext);

        for i in 0..run.len() as i32 {
            let mut one = [0.0f32; 1];
            let point = Volume::point(IVec3::new(7, ext.block_y(i), -13));
            params.eval(&mut one, &point);
            assert_eq!(one[0], run[i as usize], "at block y {}", ext.block_y(i));
        }
    }

    /// The columns of a box are sampled independently, so widening the extent
    /// must not disturb any of them.
    #[test]
    fn a_box_agrees_with_the_columns_it_covers() {
        let params = overworld();
        let ext = Volume::new(
            IVec3::new(2, 6, 2),
            IVec3::new(7, -32, -13),
            IVec3::new(3, 4, 5),
        );
        let mut whole = vec![0.0f32; ext.len()];
        params.eval(&mut whole, &ext);

        for iz in 0..2 {
            for ix in 0..2 {
                let single = Volume::new(
                    IVec3::new(1, 6, 1),
                    IVec3::new(ext.block_x(ix), -32, ext.block_z(iz)),
                    IVec3::new(1, 4, 1),
                );
                let mut run = [0.0f32; 6];
                params.eval(&mut run, &single);
                let base = ext.index_unchecked(ix, 0, iz);
                for (i, &value) in run.iter().enumerate() {
                    assert_eq!(whole[base + i].to_bits(), value.to_bits());
                }
            }
        }
    }

    #[test]
    fn every_limit_amplitude_is_an_exact_power_of_two() {
        let params = overworld();
        let expected: Vec<f32> = (0..16).map(|i| 2.0f32.powi(i - 16)).collect();
        assert_eq!(params.inner.min_limit.amplitudes(), expected);
        // 12.75 / 255 is exactly 0.05, so the main stack is exact too.
        assert_eq!(params.inner.main.layer_count(), 8);
        assert_eq!(params.inner.main.layer(0).2, 0.05);
        assert_eq!(params.inner.main.layer(7).2, 6.4);
    }

    #[test]
    fn the_main_smear_divides_by_the_y_factor_a_second_time() {
        let params = overworld();
        let inner = &params.inner;
        assert_eq!(
            inner.min_limit.layer(0).0.fudge_y_scale(),
            684.412 * 0.125 * 8.0
        );
        assert_eq!(
            inner.main.layer(0).0.fudge_y_scale(),
            (684.412 * 0.125 * 8.0) / 160.0
        );
        // Sampling scales are pre-divided by the factors, never divided per sample.
        assert_eq!(inner.xz_multiplier, 684.412 * 0.25);
        assert_eq!(inner.main_xz_multiplier, (684.412 * 0.25) / 80.0);
        assert_eq!(inner.main_y_multiplier, (684.412 * 0.125) / 160.0);
        // Each layer halves its smear together with its frequency.
        assert_eq!(
            inner.min_limit.layer(1).0.fudge_y_scale(),
            inner.min_limit.layer(0).0.fudge_y_scale() * 0.5
        );
        assert_eq!(
            inner.min_limit.layer(1).1,
            inner.min_limit.layer(0).1 * 0.5
        );
    }

    #[test]
    fn the_three_stacks_draw_from_one_stream_in_order() {
        let params = overworld();
        let min0 = params.inner.min_limit.layer(0).0;
        let max0 = params.inner.max_limit.layer(0).0;
        assert_ne!(min0, max0, "the two limit stacks must not share octaves");
        assert_ne!(min0, params.inner.main.layer(0).0);
    }

    #[test]
    fn the_output_stays_inside_the_declared_limit_range() {
        let params = overworld();
        let ext = Volume::new(
            IVec3::new(4, 8, 4),
            IVec3::new(-74, -64, -82),
            IVec3::new(37, 8, 41),
        );
        let mut out = vec![0.0f32; ext.len()];
        params.eval(&mut out, &ext);
        for v in out {
            assert!(v.abs() <= 2.1670623, "{v} escapes the published bound");
        }
    }
}
