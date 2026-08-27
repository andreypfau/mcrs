use super::*;
use crate::density_function::branch_schedule::{BranchSchedule, Step};
use crate::density_function::proto::{
    ALL_AXES, AXIS_X, AXIS_Y, AXIS_Z, Axis, ClampArguments, ConstantValue, DensityFunctionHolder,
    DistanceMetric, GradientArguments, HashableF64, InlineReference, IntervalSelectArguments,
    NoiseHolder, NoiseParam, NoiseValue, Normalization, PowFunctionArguments, ProtoDensityFunction,
    RewriteRule, RoundFunctionArguments, RoundingMode, SingleArgumentFunction, SliceUniformAxes,
    SplineHolder, TilingMode, TwoArgumentFunction, Visitor, noise_scale_axes,
};
use crate::noise::normal_noise::{ColumnScratch, NoiseSampler};
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use crate::noise::simplex::SimplexNoise;
use crate::proto::NoiseGeneratorSettings;
use bevy_math::{Curve, FloatExt, IVec3};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, RandomSource};
use mcrs_voxel_storage::VoxelId;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Formatter};
use std::mem::swap;
use std::ops::Index;
use std::sync::Arc;
use tracing::info;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RangeChoice {
    pub(crate) input_index: usize,
    pub(crate) when_in_index: usize,
    pub(crate) when_out_index: usize,
    pub(crate) min_inclusion_value: f32,
    pub(crate) max_exclusion_value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SplineValue {
    Spline(Spline),
    Constant(f32),
}

impl SplineValue {
    pub(crate) fn range(&self) -> Interval {
        match self {
            SplineValue::Spline(x) => x.range,
            SplineValue::Constant(x) => Interval::exact(*x),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Segment {
    pub(crate) left: f32,
    pub(crate) dist: f32,             // x[i+1] - x[i]
    pub(crate) lower_deriv_dist: f32, // d[i]   * dist
    pub(crate) upper_deriv_dist: f32, // d[i+1] * dist
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Spline {
    pub(crate) input_index: usize,
    pub(crate) range: Interval,
    pub(crate) locations: Box<[f32]>,
    pub(crate) derivatives: Box<[f32]>,
    pub(crate) values: Box<[SplineValue]>,
    pub(crate) segments: Box<[Segment]>, // len = locations.len() - 1
}

impl Spline {
    pub fn new(
        input_index: usize,
        coordinate: Interval,
        locations: Vec<f32>,
        derivatives: Vec<f32>,
        values: Vec<SplineValue>,
    ) -> Self {
        let n = locations.len() - 1;

        let mut min_value = f32::INFINITY;
        let mut max_value = f32::NEG_INFINITY;

        let coordinate_min = coordinate.min();
        let coordinate_max = coordinate.max();
        if coordinate_min < locations[0] {
            let extend_min = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].range().min(),
                &derivatives,
                0,
            );
            let extend_max = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].range().max(),
                &derivatives,
                0,
            );
            min_value = min_value.min(extend_min.min(extend_max));
            max_value = max_value.max(extend_min.max(extend_max));
        }

        if coordinate_max > locations[n] {
            let extend_min = Self::linear_extend(
                coordinate_max,
                &locations,
                values[n].range().min(),
                &derivatives,
                n,
            );
            let extend_max = Self::linear_extend(
                coordinate_max,
                &locations,
                values[n].range().max(),
                &derivatives,
                n,
            );
            min_value = min_value.min(extend_min.min(extend_max));
            max_value = max_value.max(extend_min.max(extend_max));
        }

        values.iter().for_each(|v| {
            min_value = min_value.min(v.range().min());
            max_value = max_value.max(v.range().max());
        });

        for i in 0..n {
            let location_left = locations[i];
            let location_right = locations[i + 1];
            let location_delta = location_right - location_left;

            let left = values[i].range();
            let right = values[i + 1].range();
            let min_left = left.min();
            let max_left = left.max();
            let min_right = right.min();
            let max_right = right.max();

            let derivative_left = derivatives[i];
            let derivative_right = derivatives[i + 1];

            if derivative_left != 0.0 || derivative_right != 0.0 {
                let max_value_delta_left = derivative_left * location_delta;
                let max_value_delta_right = derivative_right * location_delta;

                let mut local_min = min_left.min(min_right);
                let mut local_max = max_left.max(max_right);

                let min_delta_left = max_value_delta_left - max_right + min_left;
                let max_delta_left = max_value_delta_left - min_right + max_left;

                let min_delta_right = -max_value_delta_right + min_right - max_left;
                let max_delta_right = -max_value_delta_right + max_right - min_left;

                let min_delta = min_delta_left.min(min_delta_right);
                let max_delta = max_delta_left.max(max_delta_right);

                local_min = local_min.min(local_min + 0.25 * min_delta);
                local_max = local_max.max(local_max + 0.25 * max_delta);

                min_value = min_value.min(local_min);
                max_value = max_value.max(local_max);
            }
        }

        let mut segs = Vec::with_capacity(n);
        for i in 0..n {
            let left = locations[i];
            let dist = locations[i + 1] - left;
            debug_assert!(dist > 0.0, "locations must be strictly increasing");
            segs.push(Segment {
                left,
                dist,
                lower_deriv_dist: derivatives[i] * dist,
                upper_deriv_dist: derivatives[i + 1] * dist,
            });
        }

        Self {
            input_index,
            range: Interval::of(min_value, max_value),
            locations: locations.into_boxed_slice(),
            derivatives: derivatives.into_boxed_slice(),
            values: values.into_boxed_slice(),
            segments: segs.into_boxed_slice(),
        }
    }

    #[inline]
    fn linear_extend(
        point: f32,
        locations: &[f32],
        value: f32,
        derivatives: &[f32],
        i: usize,
    ) -> f32 {
        let f = derivatives[i];
        if f == 0.0 {
            value
        } else {
            value + f * (point - locations[i])
        }
    }

    #[inline(always)]
    fn upper_bound(xs: &[f32], x: f32) -> usize {
        // index of first element > x  (upper_bound)
        match xs.binary_search_by(|v| v.total_cmp(&x)) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }

    #[inline(always)]
    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + t * (b - a)
    }
}

impl SplineValue {
    #[inline]
    pub(crate) fn sample(&self, cache: &[f32]) -> f32 {
        match self {
            SplineValue::Spline(x) => x.sample(cache),
            SplineValue::Constant(x) => *x,
        }
    }
}

impl Spline {
    pub(crate) fn sample(&self, cache: &[f32]) -> f32 {
        let location = cache[self.input_index];

        let locs = &self.locations;
        let idx_gt = Self::upper_bound(locs, location);
        let n_points = locs.len();

        if idx_gt == 0 {
            let v0 = self.values[0].sample(cache);
            let d0 = self.derivatives[0];
            return if d0 == 0.0 {
                v0
            } else {
                v0 + d0 * (location - locs[0])
            };
        }

        if idx_gt == n_points {
            let i = n_points - 1;
            let v = self.values[i].sample(cache);
            let d = self.derivatives[i];
            return if d == 0.0 {
                v
            } else {
                v + d * (location - locs[i])
            };
        }

        let i0 = idx_gt - 1;
        let i1 = idx_gt;

        let v0 = self.values[i0].sample(cache);
        let v1 = self.values[i1].sample(cache);

        let seg = self.segments[i0];
        let x = (location - seg.left) / seg.dist;

        let delta = v1 - v0;

        let e0 = seg.lower_deriv_dist - delta;
        let e1 = -seg.upper_deriv_dist + delta;

        let cubic = (x * (1.0 - x)) * Self::lerp(e0, e1, x);
        let linear = Self::lerp(v0, v1, x);

        cubic + linear
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Lerp {
    pub(crate) alpha_index: usize,
    pub(crate) first_index: usize,
    pub(crate) second_index: usize,
}

impl SplineValue {
    pub(crate) fn rewrite_indices(&mut self, redirect: &[usize]) {
        if let SplineValue::Spline(spline) = self {
            spline.rewrite_indices(redirect);
        }
    }
}

impl Spline {
    pub(crate) fn rewrite_indices(&mut self, redirect: &[usize]) {
        self.input_index = redirect[self.input_index];
        for value in self.values.iter_mut() {
            value.rewrite_indices(redirect);
        }
    }

    pub(crate) fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        f(self.input_index);
        for value in self.values.iter() {
            if let SplineValue::Spline(nested) = value {
                nested.visit_input_indices(f);
            }
        }
    }
}
impl DensitySampler for RangeChoice {
    fn sample_value(&self, ctx: Fill<'_>, index: usize) -> f32 {
        let value = ctx.row(self.input_index)[index];
        if value >= self.min_inclusion_value && value < self.max_exclusion_value {
            ctx.row(self.when_in_index)[index]
        } else {
            ctx.row(self.when_out_index)[index]
        }
    }

    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let input = ctx.row(self.input_index);
        let when_in = ctx.row(self.when_in_index);
        let when_out = ctx.row(self.when_out_index);
        for (((slot, &value), &inside), &outside) in
            out.iter_mut().zip(input).zip(when_in).zip(when_out)
        {
            *slot = if value >= self.min_inclusion_value && value < self.max_exclusion_value {
                inside
            } else {
                outside
            };
        }
    }
}

impl DensitySampler for Lerp {
    fn sample_value(&self, ctx: Fill<'_>, index: usize) -> f32 {
        let alpha = ctx.row(self.alpha_index)[index];
        let first = ctx.row(self.first_index)[index];
        if alpha == 0.0 {
            return first;
        }
        let second = ctx.row(self.second_index)[index];
        if alpha == 1.0 {
            second
        } else {
            first + alpha * (second - first)
        }
    }

    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let alpha = ctx.row(self.alpha_index);
        let first = ctx.row(self.first_index);
        let second = ctx.row(self.second_index);
        for (((slot, &alpha), &first), &second) in out.iter_mut().zip(alpha).zip(first).zip(second)
        {
            *slot = if alpha == 0.0 {
                first
            } else if alpha == 1.0 {
                second
            } else {
                first + alpha * (second - first)
            };
        }
    }
}

impl DensitySampler for Spline {
    fn sample_value(&self, ctx: Fill<'_>, index: usize) -> f32 {
        let column: Vec<f32> = (0..ctx.depth()).map(|j| ctx.row(j)[index]).collect();
        self.sample(&column)
    }

    /// Reads its inputs by stack index rather than by edge, so it needs the
    /// whole register column gathered per position.
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let depth = ctx.depth();
        let mut column = vec![0.0f32; depth];
        for (p, slot) in out.iter_mut().enumerate() {
            for (j, cell) in column.iter_mut().enumerate() {
                *cell = ctx.row(j)[p];
            }
            *slot = self.sample(&column);
        }
    }
}
