use crate::interval::Interval;
use crate::jmath;
use crate::kernel::each_column;
use crate::perlin::SmearedPerlinNoise;
use crate::sample_grid::SampleGrid;
use crate::stack::{ColumnScratch, NoiseStack};
use mcrs_minecraft_numeric as numeric;
use mcrs_minecraft_random::RandomSource;
use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

thread_local! {
    static LIMITS: RefCell<LimitScratch> = RefCell::new(LimitScratch::default());
}

#[derive(Default)]
struct LimitScratch {
    min: Vec<f32>,
    max: Vec<f32>,
    read: ReadScratch,
}

#[derive(Default)]
struct ReadScratch {
    heights: Vec<i32>,
    values: Vec<f32>,
    column: ColumnScratch,
}

const BASE_SCALE: f64 = 684.412;
const LIMIT_FACTOR: f64 = 0.99998474f32 as f64;
const MAIN_FACTOR: f64 = 12.75;
const LIMIT_FIRST_OCTAVE: i32 = -15;
const MAIN_FIRST_OCTAVE: i32 = -7;

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

        let min_limit = create_fbm(
            random,
            LIMIT_FIRST_OCTAVE,
            limit_smear_scale_y,
            LIMIT_FACTOR,
        );
        let max_limit = create_fbm(
            random,
            LIMIT_FIRST_OCTAVE,
            limit_smear_scale_y,
            LIMIT_FACTOR,
        );
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

    pub fn range(&self) -> Interval {
        self.inner.range
    }

    pub fn eval(&self, out: &mut [f32], ext: &SampleGrid) {
        let n = ext.size().y as usize;
        let inner = &*self.inner;

        LIMITS.with_borrow_mut(|limits| {
            let LimitScratch { min, max, read } = limits;
            min.clear();
            min.resize(n, 0.0);
            max.clear();
            max.resize(n, 0.0);
            each_column(out, ext, |run, ix, iz| {
                let column = LimitColumn {
                    inner,
                    ext,
                    block_x: ext.block_x(ix as i32),
                    block_z: ext.block_z(iz as i32),
                };

                inner.fbm.main.fill_column_at(
                    run,
                    column.block_x,
                    column.block_z,
                    ext,
                    inner.main_xz_multiplier,
                    inner.main_y_multiplier,
                    &mut read.column,
                );
                for alpha in run.iter_mut() {
                    *alpha = numeric::clamp_f32(*alpha + 0.5, 0.0, 1.0);
                }
                column.fill_where_read(&inner.fbm.min_limit, 1.0, run, min, read);
                column.fill_where_read(&inner.fbm.max_limit, 0.0, run, max, read);

                for ((slot, &min), &max) in run.iter_mut().zip(&*min).zip(&*max) {
                    let alpha = *slot;
                    *slot = if alpha == 0.0 {
                        min
                    } else if alpha == 1.0 {
                        max
                    } else {
                        jmath::lerp(alpha, min, max)
                    };
                }
            });
        });
    }
}

struct LimitColumn<'a> {
    inner: &'a Inner,
    ext: &'a SampleGrid,
    block_x: i32,
    block_z: i32,
}

impl LimitColumn<'_> {
    fn fill_where_read(
        &self,
        limit: &NoiseStack<SmearedPerlinNoise>,
        unread: f32,
        alphas: &[f32],
        out: &mut [f32],
        scratch: &mut ReadScratch,
    ) {
        let read_rows = (0i32..).zip(alphas).filter(|(_, alpha)| **alpha != unread);
        scratch.heights.clear();
        scratch
            .heights
            .extend(read_rows.map(|(row, _)| self.ext.block_y(row)));
        if scratch.heights.is_empty() {
            return;
        }
        scratch.values.clear();
        scratch.values.resize(scratch.heights.len(), 0.0);
        limit.fill_heights_at(
            &mut scratch.values,
            self.block_x,
            self.block_z,
            &scratch.heights,
            self.inner.xz_multiplier,
            self.inner.y_multiplier,
            &mut scratch.column,
        );
        let read_slots = out
            .iter_mut()
            .zip(alphas)
            .filter(|(_, alpha)| **alpha != unread);
        for ((slot, _), &value) in read_slots.zip(&scratch.values) {
            *slot = value;
        }
    }
}

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
    use glam::IVec3;

    fn column() -> SampleGrid {
        SampleGrid::new(
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
            let point = SampleGrid::point(IVec3::new(7, ext.block_y(i), -13));
            params.eval(&mut one, &point);
            assert_eq!(one[0], run[i as usize], "at block y {}", ext.block_y(i));
        }
    }

    #[test]
    fn a_box_agrees_with_the_columns_it_covers() {
        let params = overworld();
        let ext = SampleGrid::new(
            IVec3::new(2, 6, 2),
            IVec3::new(7, -32, -13),
            IVec3::new(3, 4, 5),
        );
        let mut whole = vec![0.0f32; ext.len()];
        params.eval(&mut whole, &ext);

        for iz in 0..2 {
            for ix in 0..2 {
                let single = SampleGrid::new(
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
        assert_eq!(inner.xz_multiplier, 684.412 * 0.25);
        assert_eq!(inner.main_xz_multiplier, (684.412 * 0.25) / 80.0);
        assert_eq!(inner.main_y_multiplier, (684.412 * 0.125) / 160.0);
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
        let ext = SampleGrid::new(
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

    #[test]
    fn a_column_crossing_both_factor_bounds_matches_the_full_limit_columns() {
        let params = overworld();
        let inner = &*params.inner;
        let ext = SampleGrid::new(
            IVec3::new(1, 49, 1),
            IVec3::new(0, -64, 0),
            IVec3::new(1, 8, 1),
        );
        let mut scratch = ColumnScratch::default();
        let mut fill = |stack: &NoiseStack<SmearedPerlinNoise>, xz: f64, y: f64| {
            let mut column = vec![0.0f32; ext.len()];
            stack.fill_column_at(&mut column, 0, 0, &ext, xz, y, &mut scratch);
            column
        };
        let alphas: Vec<f32> = fill(
            &inner.fbm.main,
            inner.main_xz_multiplier,
            inner.main_y_multiplier,
        )
        .into_iter()
        .map(|value| (value + 0.5).clamp(0.0, 1.0))
        .collect();
        let min = fill(
            &inner.fbm.min_limit,
            inner.xz_multiplier,
            inner.y_multiplier,
        );
        let max = fill(
            &inner.fbm.max_limit,
            inner.xz_multiplier,
            inner.y_multiplier,
        );
        for bound in [0.0, 1.0] {
            assert!(
                alphas.contains(&bound),
                "the column must reach factor {bound}"
            );
        }
        assert!(
            alphas.iter().any(|alpha| *alpha > 0.0 && *alpha < 1.0),
            "the column must hold a fractional factor"
        );

        let mut run = vec![0.0f32; ext.len()];
        params.eval(&mut run, &ext);
        for (row, &value) in run.iter().enumerate() {
            let expected = match alphas[row] {
                0.0 => min[row],
                1.0 => max[row],
                alpha => jmath::lerp(alpha, min[row], max[row]),
            };
            assert_eq!(value.to_bits(), expected.to_bits(), "at row {row}");
        }
    }
}
