use crate::interval::Interval;
use crate::jmath;
use crate::kernel::each_column;
use crate::noise::perlin::SmearedPerlinNoise;
use crate::noise::stack::{ColumnScratch, NoiseStack};
use crate::volume::Volume;
use mcrs_minecraft_random::RandomSource;
use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

pub const NOISE_SEED: &str = "minecraft:terrain";

thread_local! {
    static LIMITS: RefCell<(Vec<f32>, ColumnScratch)> = RefCell::new(Default::default());
}

const BASE_SCALE: f64 = 684.412;
/// Java writes the float literal `0.99998474F` into a `double` field, so the
/// widened value is exactly `(2^16 - 1) / 2^16` and every limit amplitude lands
/// on an exact power of two.
const LIMIT_FACTOR: f64 = 0.99998474f32 as f64;
const MAIN_FACTOR: f64 = 12.75;
const LIMIT_FIRST_OCTAVE: i32 = -15;
const MAIN_FIRST_OCTAVE: i32 = -7;

/// The three fbm stacks one `BlendedNoise` draws, in stream order. Vanilla's
/// `BlendedNoise.FbmSet`.
struct FbmSet {
    min_limit: NoiseStack<SmearedPerlinNoise>,
    max_limit: NoiseStack<SmearedPerlinNoise>,
    main: NoiseStack<SmearedPerlinNoise>,
}

struct Inner {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
    range: Interval,
    xz_multiplier: f64,
    y_multiplier: f64,
    main_xz_multiplier: f64,
    main_y_multiplier: f64,
    fbm: FbmSet,
}

/// Vanilla's `BlendedNoise.createFbm` schedule: octaves highest frequency
/// first, which is the order they draw from the stream, each halving both its
/// frequency and its smear scale while its weight doubles.
fn fbm_layers(first_octave: i32, value_factor: f64) -> impl Iterator<Item = (f64, f64)> {
    let octaves = -first_octave + 1;
    let mut frequency = 1.0f64;
    let mut value_factor = value_factor / (2.0f64.powi(octaves) - 1.0);
    (0..octaves).map(move |_| {
        let layer = (frequency, value_factor);
        frequency *= 0.5;
        value_factor *= 2.0;
        layer
    })
}

fn create_fbm(
    random: &mut RandomSource,
    first_octave: i32,
    smear_scale_y: f64,
    value_factor: f64,
) -> NoiseStack<SmearedPerlinNoise> {
    let mut stack = NoiseStack::builder();
    for (frequency, value_factor) in fbm_layers(first_octave, value_factor) {
        stack.add(
            SmearedPerlinNoise::from_random(random, smear_scale_y * frequency),
            frequency,
            value_factor as f32,
        );
    }
    stack.build()
}

/// `BlendedNoise.range()`. The two limit stacks are what escapes furthest and
/// the main stack only picks between them, so the bound is one limit fbm.
///
/// The per-layer bound is the smeared noise's `±(|fudge_y_scale| + 2.0)`, not a
/// plain Perlin's flat `±2.0`.
fn declared_range(y_scale: f64, smear_scale_multiplier: f64) -> Interval {
    let smear_scale_y = (BASE_SCALE * y_scale) * smear_scale_multiplier;
    let mut range = Interval::exact(0.0);
    for (frequency, value_factor) in fbm_layers(LIMIT_FIRST_OCTAVE, LIMIT_FACTOR) {
        range = range
            + Interval::symmetric(((smear_scale_y * frequency).abs() + 2.0) as f32)
                * Interval::exact(value_factor as f32);
    }
    range
}

#[derive(Clone)]
pub struct BlendedNoise {
    inner: Arc<Inner>,
}

impl BlendedNoise {
    pub fn new(
        random: &mut RandomSource,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
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
                range: declared_range(y_scale, smear_scale_multiplier),
                xz_multiplier,
                y_multiplier,
                main_xz_multiplier: xz_multiplier / xz_factor,
                main_y_multiplier: y_multiplier / y_factor,
                fbm: FbmSet {
                    min_limit,
                    max_limit,
                    main,
                },
            }),
        }
    }

    /// `BlendedNoise.range()`.
    pub fn range(&self) -> Interval {
        self.inner.range
    }

    pub fn eval(&self, out: &mut [f32], ext: &Volume) {
        let n = ext.size().y as usize;
        let inner = &*self.inner;

        LIMITS.with_borrow_mut(|(floats, scratch)| {
            floats.clear();
            floats.resize(2 * n, 0.0);
            each_column(out, ext, |run, ix, iz| {
                let (min, max) = floats.split_at_mut(n);
                let bx = ext.block_x(ix as i32);
                let bz = ext.block_z(iz as i32);

                inner.fbm.main.fill_column_at(
                    run,
                    bx,
                    bz,
                    ext,
                    inner.main_xz_multiplier,
                    inner.main_y_multiplier,
                    scratch,
                );
                for alpha in run.iter_mut() {
                    *alpha = jmath::clampf(*alpha + 0.5, 0.0, 1.0);
                }
                // An alpha pinned to an endpoint returns one limit untouched, so the
                // other stack is never read and its sixteen octaves need not be
                // sampled. The test is per column, which is where it pays.
                if run.iter().any(|&alpha| alpha != 1.0) {
                    inner.fbm.min_limit.fill_column_at(
                        min,
                        bx,
                        bz,
                        ext,
                        inner.xz_multiplier,
                        inner.y_multiplier,
                        scratch,
                    );
                }
                if run.iter().any(|&alpha| alpha != 0.0) {
                    inner.fbm.max_limit.fill_column_at(
                        max,
                        bx,
                        bz,
                        ext,
                        inner.xz_multiplier,
                        inner.y_multiplier,
                        scratch,
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
            });
        });
    }
}

/// Hand-written because the derived form would print forty octaves of
/// 256-byte permutation table.
impl fmt::Debug for BlendedNoise {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlendedNoise")
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

    fn overworld() -> BlendedNoise {
        let mut random = RandomSource::new(42, false);
        BlendedNoise::new(&mut random, 0.25, 0.125, 80.0, 160.0, 8.0)
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
        assert_eq!(params.inner.fbm.min_limit.amplitudes(), expected);
        // 12.75 / 255 is exactly 0.05, so the main stack is exact too.
        assert_eq!(params.inner.fbm.main.layer_count(), 8);
        assert_eq!(params.inner.fbm.main.layer(0).2, 0.05);
        assert_eq!(params.inner.fbm.main.layer(7).2, 6.4);
    }

    #[test]
    fn the_main_smear_divides_by_the_y_factor_a_second_time() {
        let params = overworld();
        let inner = &params.inner;
        assert_eq!(
            inner.fbm.min_limit.layer(0).0.fudge_y_scale(),
            684.412 * 0.125 * 8.0
        );
        assert_eq!(
            inner.fbm.main.layer(0).0.fudge_y_scale(),
            (684.412 * 0.125 * 8.0) / 160.0
        );
        // Sampling scales are pre-divided by the factors, never divided per sample.
        assert_eq!(inner.xz_multiplier, 684.412 * 0.25);
        assert_eq!(inner.main_xz_multiplier, (684.412 * 0.25) / 80.0);
        assert_eq!(inner.main_y_multiplier, (684.412 * 0.125) / 160.0);
        // Each layer halves its smear together with its frequency.
        assert_eq!(
            inner.fbm.min_limit.layer(1).0.fudge_y_scale(),
            inner.fbm.min_limit.layer(0).0.fudge_y_scale() * 0.5
        );
        assert_eq!(
            inner.fbm.min_limit.layer(1).1,
            inner.fbm.min_limit.layer(0).1 * 0.5
        );
    }

    #[test]
    fn the_three_stacks_draw_from_one_stream_in_order() {
        let params = overworld();
        let min0 = params.inner.fbm.min_limit.layer(0).0;
        let max0 = params.inner.fbm.max_limit.layer(0).0;
        assert_ne!(min0, max0, "the two limit stacks must not share octaves");
        assert_ne!(min0, params.inner.fbm.main.layer(0).0);
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
