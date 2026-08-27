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
use crate::spline::{RangeFunction, SplineFunction};
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
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for RangeChoice {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SplineValue {
    Spline(Spline),
    Constant(f32),
}

impl RangeFunction for SplineValue {
    fn min_value(&self) -> f32 {
        match self {
            SplineValue::Spline(x) => x.min_value(),
            SplineValue::Constant(x) => *x,
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            SplineValue::Spline(x) => x.max_value(),
            SplineValue::Constant(x) => *x,
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
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
    pub(crate) locations: Box<[f32]>,
    pub(crate) derivatives: Box<[f32]>,
    pub(crate) values: Box<[SplineValue]>,
    pub(crate) segments: Box<[Segment]>, // len = locations.len() - 1
}

impl Spline {
    pub fn new(
        input_index: usize,
        coordinate_min: f32,
        coordinate_max: f32,
        locations: Vec<f32>,
        derivatives: Vec<f32>,
        values: Vec<SplineValue>,
    ) -> Self {
        let n = locations.len() - 1;

        let mut min_value = f32::INFINITY;
        let mut max_value = f32::NEG_INFINITY;

        if coordinate_min < locations[0] {
            let extend_min = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].min_value(),
                &derivatives,
                0,
            );
            let extend_max = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].max_value(),
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
                values[n].min_value(),
                &derivatives,
                n,
            );
            let extend_max = Self::linear_extend(
                coordinate_max,
                &locations,
                values[n].max_value(),
                &derivatives,
                n,
            );
            min_value = min_value.min(extend_min.min(extend_max));
            max_value = max_value.max(extend_min.max(extend_max));
        }

        values.iter().for_each(|v| {
            min_value = min_value.min(v.min_value());
            max_value = max_value.max(v.max_value());
        });

        for i in 0..n {
            let location_left = locations[i];
            let location_right = locations[i + 1];
            let location_delta = location_right - location_left;

            let min_left = values[i].min_value();
            let max_left = values[i].max_value();
            let min_right = values[i + 1].min_value();
            let max_right = values[i + 1].max_value();

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
            min_value,
            max_value,
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

impl RangeFunction for Spline {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
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
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for Lerp {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
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
