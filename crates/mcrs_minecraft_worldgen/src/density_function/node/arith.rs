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
pub(crate) struct Linear {
    pub(crate) input_index: usize,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
    pub(crate) argument: f32,
    pub(crate) operation: LinearOperation,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Affine {
    pub(crate) input_index: usize,
    pub(crate) scale: f32,
    pub(crate) offset: f32,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
pub(crate) enum LinearOperation {
    Add,
    Multiply,
}

impl Affine {
    pub(crate) fn compute_range(
        input_min: f32,
        input_max: f32,
        scale: f32,
        offset: f32,
    ) -> (f32, f32) {
        if scale >= 0.0 {
            (
                input_min.mul_add(scale, offset),
                input_max.mul_add(scale, offset),
            )
        } else {
            (
                input_max.mul_add(scale, offset),
                input_min.mul_add(scale, offset),
            )
        }
    }
}

impl RangeFunction for Affine {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

/// Piecewise-linear affine: different scales for negative vs non-negative input.
///
/// Replaces patterns like `Affine(Unary::QuarterNegative(x))` or
/// `Affine(Unary::HalfNegative(x))` where the unary damps the negative side.
///
/// Computes: `if x < 0 { x * neg_scale + offset } else { x * pos_scale + offset }`
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PiecewiseAffine {
    pub(crate) input_index: usize,
    pub(crate) neg_scale: f32,
    pub(crate) pos_scale: f32,
    pub(crate) offset: f32,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl PiecewiseAffine {
    pub(crate) fn compute_range(
        input_min: f32,
        input_max: f32,
        neg_scale: f32,
        pos_scale: f32,
        offset: f32,
    ) -> (f32, f32) {
        // Two monotone pieces meeting at zero: the extremes can only sit at an
        // endpoint or at the breakpoint, and the breakpoint only counts when the
        // input interval actually straddles it.
        let apply = |x: f32| {
            if x < 0.0 {
                x * neg_scale + offset
            } else {
                x * pos_scale + offset
            }
        };
        let a = apply(input_min);
        let b = apply(input_max);
        let mut lo = a.min(b);
        let mut hi = a.max(b);
        if input_min < 0.0 && input_max >= 0.0 {
            lo = lo.min(offset);
            hi = hi.max(offset);
        }
        (lo, hi)
    }
}

impl RangeFunction for PiecewiseAffine {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

/// Fused world-boundary "slide" operation.
///
/// Replaces the 5-node chain:
///   `Affine(+a) → Mul(y_grad1) → Affine(+b) → Mul(y_grad2) → Affine(+c)`
///
/// Full computation:
///   `grad2(y) * (grad1(y) * (input + offset_a) + offset_b) + offset_c`
///
/// Fast path: when both gradients saturate to 1.0 (y in the interior range),
/// the three offsets cancel out and the result equals `input + combined_offset`
/// (which is typically ~0, i.e. identity).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slide {
    pub(crate) input_index: usize,

    // First Y-gradient applied (typically "top": 240..256 → 1.0..0.0)
    pub(crate) grad1: ClampedYGradient,
    // Second Y-gradient applied (typically "bottom": -64..-40 → 0.0..1.0)
    pub(crate) grad2: ClampedYGradient,

    // Three affine offsets (all original affines had scale=1.0)
    pub(crate) offset_a: f32, // pre-grad1
    pub(crate) offset_b: f32, // between grad1 and grad2
    pub(crate) offset_c: f32, // post-grad2

    // Pre-computed: offset_a + offset_b + offset_c
    pub(crate) combined_offset: f32,

    // Y range where both gradients saturate to 1.0 (fast path)
    pub(crate) fast_path_min_y: f32,
    pub(crate) fast_path_max_y: f32,

    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl Slide {
    #[inline]
    fn eval_gradient(g: &ClampedYGradient, y: f32) -> f32 {
        if y < g.from_y {
            g.from_value
        } else if y > g.to_y {
            g.to_value
        } else {
            g.from_value + (g.to_value - g.from_value) * (y - g.from_y) / (g.to_y - g.from_y)
        }
    }

    #[inline]
    pub(crate) fn compute(&self, input: f32, y: f32) -> f32 {
        if y > self.fast_path_min_y && y < self.fast_path_max_y {
            input + self.combined_offset
        } else {
            let g1 = Self::eval_gradient(&self.grad1, y);
            let g2 = Self::eval_gradient(&self.grad2, y);
            (g1 * (input + self.offset_a) + self.offset_b).mul_add(g2, self.offset_c)
        }
    }

    /// Compute the Y range where a gradient saturates to exactly 1.0.
    /// Returns (min_y, max_y) or None if the gradient never equals 1.0.
    pub(crate) fn saturate_one_range(g: &ClampedYGradient) -> Option<(f32, f32)> {
        let below = g.from_value == 1.0; // y <= from_y → 1.0
        let above = g.to_value == 1.0; // y >= to_y → 1.0
        match (below, above) {
            (true, true) => Some((f32::NEG_INFINITY, f32::INFINITY)),
            (true, false) => Some((f32::NEG_INFINITY, g.from_y)),
            (false, true) => Some((g.to_y, f32::INFINITY)),
            (false, false) => None,
        }
    }
}

impl RangeFunction for Slide {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl RangeFunction for Linear {
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
pub(crate) struct Unary {
    pub(crate) input_index: usize,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
    pub(crate) operation: UnaryOperation,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
pub(crate) enum UnaryOperation {
    Abs,
    Square,
    Cube,
    HalfNegative,
    QuarterNegative,
    Reciprocal,
    Squeeze,
    Sqrt,
    Log,
    Sign,
}

impl UnaryOperation {
    #[inline]
    pub fn apply(&self, value: f32) -> f32 {
        match self {
            UnaryOperation::Abs => value.abs(),
            UnaryOperation::Square => value.powi(2),
            UnaryOperation::Cube => value.powi(3),
            UnaryOperation::HalfNegative => {
                if value > 0.0 {
                    value
                } else {
                    value * 0.5
                }
            }
            UnaryOperation::QuarterNegative => {
                if value > 0.0 {
                    value
                } else {
                    value * 0.25
                }
            }
            UnaryOperation::Reciprocal => 1.0 / value,
            UnaryOperation::Squeeze => {
                let clamped = value.clamp(-1.0, 1.0);
                clamped / 2.0 - clamped.powi(3) / 24.0
            }
            UnaryOperation::Sqrt => value.sqrt(),
            UnaryOperation::Log => (value as f64).ln() as f32,
            // Unlike f32::signum, zero and NaN come back unchanged.
            UnaryOperation::Sign => {
                if value == 0.0 || value.is_nan() {
                    value
                } else if value > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}

impl RangeFunction for Unary {
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
pub(crate) struct Clamp {
    pub(crate) input_index: usize,
    /// The datapack bounds already narrowed to the input's own range. Sampling clamps
    /// to these rather than the raw bounds: for any value the input can produce the two
    /// agree, so the narrower pair serves as both the operation and the declared range.
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for Clamp {
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
pub(crate) struct Binary {
    pub(crate) input1_index: usize,
    pub(crate) input2_index: usize,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
    pub(crate) operation: BinaryOperation,
}

impl RangeFunction for Binary {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
pub(crate) enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Min,
    Max,
    Pow,
    Round(RoundingMode),
}

impl BinaryOperation {
    #[inline]
    pub(crate) fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            BinaryOperation::Add => a + b,
            BinaryOperation::Subtract => a - b,
            BinaryOperation::Multiply => a * b,
            BinaryOperation::Divide => {
                if a == 0.0 {
                    0.0
                } else {
                    a / b
                }
            }
            BinaryOperation::Min => a.min(b),
            BinaryOperation::Max => a.max(b),
            BinaryOperation::Pow => pow_narrowed(a, b),
            BinaryOperation::Round(mode) => {
                if b == 0.0 {
                    a
                } else {
                    round_to_integer(a / b, mode) * b
                }
            }
        }
    }
}

impl DensitySampler for Affine {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        for (slot, &value) in out.iter_mut().zip(ctx.row(self.input_index)) {
            *slot = value.mul_add(self.scale, self.offset);
        }
    }
}

impl DensitySampler for PiecewiseAffine {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        for (slot, &value) in out.iter_mut().zip(ctx.row(self.input_index)) {
            let scale = if value < 0.0 {
                self.neg_scale
            } else {
                self.pos_scale
            };
            *slot = value.mul_add(scale, self.offset);
        }
    }
}

impl DensitySampler for Slide {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let input = ctx.row(self.input_index);
        for ((slot, &value), pos) in out.iter_mut().zip(input).zip(ctx.positions) {
            *slot = self.compute(value, pos.y as f32);
        }
    }
}

impl DensitySampler for Clamp {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        for (slot, &value) in out.iter_mut().zip(ctx.row(self.input_index)) {
            *slot = value.clamp(self.min_value, self.max_value);
        }
    }
}

/// `min(x, c)`, `max(x, c)`, `c - x` and `c / x`: the constant-operand forms
/// the reference gives their own samplers, so no constant row is materialised
/// and the inner loop carries no branch on the operation.
macro_rules! const_binary_samplers {
    ($($(#[$doc:meta])* $name:ident, $value:ident, $arg:ident => $body:expr;)*) => {$(
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq)]
        pub(crate) struct $name {
            pub(crate) input_index: usize,
            pub(crate) argument: f32,
            pub(crate) min_value: f32,
            pub(crate) max_value: f32,
        }

        impl RangeFunction for $name {
            #[inline]
            fn min_value(&self) -> f32 {
                self.min_value
            }
            #[inline]
            fn max_value(&self) -> f32 {
                self.max_value
            }
        }

        impl DensitySampler for $name {
            fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
                let $arg = self.argument;
                for (slot, &$value) in out.iter_mut().zip(ctx.row(self.input_index)) {
                    *slot = $body;
                }
            }
        }
    )*};
}

const_binary_samplers! {
    ConstMin, value, argument => value.min(argument);
    ConstMax, value, argument => value.max(argument);
    ConstSub, value, argument => argument - value;
    ConstDiv, value, argument => if argument == 0.0 { 0.0 } else { argument / value };
}

macro_rules! two_input_samplers {
    ($($(#[$m:meta])* $name:ident, $a:ident, $b:ident => $body:expr;)*) => {$(
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq)]
        pub(crate) struct $name {
            pub(crate) input1_index: usize,
            pub(crate) input2_index: usize,
            pub(crate) min_value: f32,
            pub(crate) max_value: f32,
        }

        impl RangeFunction for $name {
            #[inline]
            fn min_value(&self) -> f32 { self.min_value }
            #[inline]
            fn max_value(&self) -> f32 { self.max_value }
        }

        impl DensitySampler for $name {
            fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
                let left = ctx.row(self.input1_index);
                let right = ctx.row(self.input2_index);
                for ((slot, &$a), &$b) in out.iter_mut().zip(left).zip(right) {
                    *slot = $body;
                }
            }
        }
    )*};
}

macro_rules! one_input_samplers {
    ($($(#[$m:meta])* $name:ident, $v:ident => $body:expr;)*) => {$(
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq)]
        pub(crate) struct $name {
            pub(crate) input_index: usize,
            pub(crate) min_value: f32,
            pub(crate) max_value: f32,
        }

        impl RangeFunction for $name {
            #[inline]
            fn min_value(&self) -> f32 { self.min_value }
            #[inline]
            fn max_value(&self) -> f32 { self.max_value }
        }

        impl DensitySampler for $name {
            fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
                for (slot, &$v) in out.iter_mut().zip(ctx.row(self.input_index)) {
                    *slot = $body;
                }
            }
        }
    )*};
}

two_input_samplers! {
    Add, a, b => a + b;
    Sub, a, b => a - b;
    Mul, a, b => a * b;
    /// A zero numerator short-circuits, so a zero divisor cannot leak a NaN.
    Div, a, b => if a == 0.0 { 0.0 } else { a / b };
    Min, a, b => a.min(b);
    Max, a, b => a.max(b);
    Pow, a, b => pow_narrowed(a, b);
}

one_input_samplers! {
    Abs, v => v.abs();
    Square, v => v.powi(2);
    Cube, v => v.powi(3);
    Negate, v => -v;
    Reciprocal, v => 1.0 / v;
    Sqrt, v => v.sqrt();
    Log, v => (v as f64).ln() as f32;
    /// Unlike `f32::signum`, zero and NaN come back unchanged.
    Sign, v => if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { v };
    Squeeze, v => {
        let clamped = v.clamp(-1.0, 1.0);
        clamped / 2.0 - clamped.powi(3) / 24.0
    };
}

const_binary_samplers! {
    ConstAdd, value, argument => value + argument;
    ConstMul, value, argument => value * argument;
}

/// Negative values are scaled, positive ones pass through.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LeakyReLU {
    pub(crate) input_index: usize,
    pub(crate) negative_factor: f32,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for LeakyReLU {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }
    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensitySampler for LeakyReLU {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let factor = self.negative_factor;
        for (slot, &v) in out.iter_mut().zip(ctx.row(self.input_index)) {
            *slot = if v > 0.0 { v } else { v * factor };
        }
    }
}

/// `x` rounded to the nearest multiple of another function's value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Round {
    pub(crate) input1_index: usize,
    pub(crate) input2_index: usize,
    pub(crate) mode: RoundingMode,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for Round {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }
    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensitySampler for Round {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let left = ctx.row(self.input1_index);
        let right = ctx.row(self.input2_index);
        let mode = self.mode;
        for ((slot, &a), &b) in out.iter_mut().zip(left).zip(right) {
            *slot = if b == 0.0 {
                a
            } else {
                round_to_integer(a / b, mode) * b
            };
        }
    }
}

/// `x` rounded to a multiple fixed at compile time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IntegerMultipleRound {
    pub(crate) input_index: usize,
    pub(crate) multiple: f32,
    pub(crate) mode: RoundingMode,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for IntegerMultipleRound {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }
    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensitySampler for IntegerMultipleRound {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let multiple = self.multiple;
        let mode = self.mode;
        for (slot, &v) in out.iter_mut().zip(ctx.row(self.input_index)) {
            *slot = if multiple == 0.0 {
                v
            } else {
                round_to_integer(v / multiple, mode) * multiple
            };
        }
    }
}

/// `x` raised to an exponent fixed at compile time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConstExponentPow {
    pub(crate) input_index: usize,
    pub(crate) exponent: f32,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for ConstExponentPow {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }
    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensitySampler for ConstExponentPow {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let exponent = self.exponent;
        for (slot, &v) in out.iter_mut().zip(ctx.row(self.input_index)) {
            *slot = pow_narrowed(v, exponent);
        }
    }
}

/// A base fixed at compile time raised to `x`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConstBasePow {
    pub(crate) input_index: usize,
    pub(crate) base: f32,
    pub(crate) min_value: f32,
    pub(crate) max_value: f32,
}

impl RangeFunction for ConstBasePow {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }
    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensitySampler for ConstBasePow {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let base = self.base;
        for (slot, &v) in out.iter_mut().zip(ctx.row(self.input_index)) {
            *slot = pow_narrowed(base, v);
        }
    }
}
