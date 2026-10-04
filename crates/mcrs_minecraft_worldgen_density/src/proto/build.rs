use crate::node::distance::DistanceMetric;
use crate::node::gradient::Tiling;
use crate::proto::args::{MAX_SURFACE_LOWER_BOUND, MIN_SURFACE_LOWER_BOUND};
use crate::proto::{
    BlendedNoiseArguments, ClampArguments, ConstantValue, DensityFunctionHolder,
    FindTopSurfaceArguments, GradientArguments, IntervalSelectArguments,
    MAX_REASONABLE_NOISE_VALUE, NoiseValue, ProtoDensityFunction as P, ProtoSpline, ScaleValue,
    SingleArgumentFunction, SmearScaleMultiplier, TwoArgumentFunction,
};
use crate::router::NoiseSettings;
use mcrs_minecraft_core::{Axis, ResourceLocation};
use mcrs_minecraft_worldgen_noise::proto::{HashableF64, NoiseHolder};
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::ops::{Add, Bound, Div, Mul, RangeBounds, RangeInclusive, Sub};

pub type Df = DensityFunctionHolder;

fn id(name: &str) -> ResourceLocation {
    ResourceLocation::read(name).expect("a valid id")
}

fn owned(function: P) -> Df {
    Df::Owned(Box::new(function))
}

impl From<f64> for Df {
    fn from(value: f64) -> Self {
        Df::Value(ConstantValue::from(value))
    }
}

impl From<f32> for Df {
    fn from(value: f32) -> Self {
        Df::Value(ConstantValue {
            value: value.into(),
        })
    }
}

impl From<&Df> for Df {
    fn from(function: &Df) -> Self {
        function.clone()
    }
}

impl From<ProtoSpline> for Df {
    fn from(spline: ProtoSpline) -> Self {
        owned(P::Spline { spline })
    }
}

/// Each operator builds the node of the same name, operands in the order
/// written: `a - b` is `sub`, never `add` of a negated constant.
macro_rules! operators {
    ($($op:ident $method:ident => $variant:ident,)*) => {$(
        impl<R: Into<Df>> $op<R> for Df {
            type Output = Df;

            fn $method(self, right: R) -> Df {
                owned(P::$variant(TwoArgumentFunction {
                    left: self,
                    right: right.into(),
                }))
            }
        }

        impl<R: Into<Df>> $op<R> for &Df {
            type Output = Df;

            fn $method(self, right: R) -> Df {
                self.clone().$method(right)
            }
        }

        impl $op<Df> for f64 {
            type Output = Df;

            fn $method(self, right: Df) -> Df {
                Df::from(self).$method(right)
            }
        }

        impl $op<&Df> for f64 {
            type Output = Df;

            fn $method(self, right: &Df) -> Df {
                Df::from(self).$method(right)
            }
        }
    )*};
}

operators! {
    Add add => Add,
    Sub sub => Sub,
    Mul mul => Mul,
    Div div => Div,
}

macro_rules! unary {
    ($($name:ident => $variant:ident,)*) => {$(
        pub fn $name(self) -> Self {
            owned(P::$variant(SingleArgumentFunction { input: self }))
        }
    )*};
}

impl DensityFunctionHolder {
    pub const ZERO: Self = Self::Value(ConstantValue {
        value: NoiseValue(0.0),
    });

    unary! {
        abs => Abs,
        square => Square,
        cube => Cube,
        half_negative => HalfNegative,
        quarter_negative => QuarterNegative,
        negate => Negate,
        reciprocal => Reciprocal,
        squeeze => Squeeze,
        cache => Cache,
        blend_density => BlendDensity,
    }

    /// An id without a namespace is in `minecraft`.
    pub fn reference(name: &str) -> Self {
        Self::Reference(id(name))
    }

    pub fn min(self, other: impl Into<Self>) -> Self {
        owned(P::Min(TwoArgumentFunction {
            left: self,
            right: other.into(),
        }))
    }

    pub fn max(self, other: impl Into<Self>) -> Self {
        owned(P::Max(TwoArgumentFunction {
            left: self,
            right: other.into(),
        }))
    }

    pub fn clamp(self, min: f32, max: f32) -> Self {
        owned(P::Clamp(ClampArguments {
            input: self,
            min: min.into(),
            max: max.into(),
        }))
    }

    pub fn interpolated(self, cell_size_xz: u32, cell_size_y: u32) -> Self {
        owned(P::Interpolated {
            input: self,
            cell_size_xz: NonZeroU32::new(cell_size_xz).expect("a cell has a width"),
            cell_size_y: NonZeroU32::new(cell_size_y).expect("a cell has a height"),
        })
    }

    pub fn slice_y(self, y: i32) -> Self {
        owned(P::Slice {
            axis: Axis::Y,
            coordinate: y,
            input: self,
        })
    }

    /// The factor and offset are folded in `f32`, which is why the game ships
    /// `-0.34999996` where the mapping reads `-0.35`.
    pub fn remap(self, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> Self {
        let factor = (to_max - to_min) / (from_max - from_min);
        let offset = to_min - from_min * factor;
        let scaled = self * factor;
        if offset == 0.0 {
            scaled
        } else {
            scaled + offset
        }
    }

    pub fn clamped_map(self, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> Self {
        self.clamp(from_min, from_max)
            .remap(from_min, from_max, to_min, to_max)
    }

    /// This function where terrain is new, `blending_target` where it meets
    /// chunks of an older generator.
    pub fn blended(self, blending_target: impl Into<Self>) -> Self {
        Self::lerp(Self::blend_alpha(), blending_target, self).cache()
    }

    /// This function inside `y_range`, `out_of_range` everywhere else.
    pub fn y_limited(self, y: &Self, y_range: RangeInclusive<i32>, out_of_range: f32) -> Self {
        let range = *y_range.start() as f32..(*y_range.end() + 1) as f32;
        Self::range_choice(y, range, self, out_of_range)
    }

    /// Fades this function into a fixed target near the top and the bottom of
    /// `bounds`. Each edge is `(start, end, target)`, the two offsets measured
    /// inwards from that edge: the function is untouched at `start` and is the
    /// target at `end`.
    pub fn slide(
        self,
        bounds: NoiseSettings,
        top: (i32, i32, f32),
        bottom: (i32, i32, f32),
    ) -> Self {
        let top_y = bounds.min_y + bounds.height as i32;
        let top_factor = Self::y_gradient(top_y - top.0, top_y - top.1, 1.0, 0.0);
        let bottom_factor =
            Self::y_gradient(bounds.min_y + bottom.0, bounds.min_y + bottom.1, 0.0, 1.0);
        Self::lerp(bottom_factor, bottom.2, Self::lerp(top_factor, top.2, self))
    }

    pub fn blend_alpha() -> Self {
        owned(P::BlendAlpha)
    }

    pub fn blend_offset() -> Self {
        owned(P::BlendOffset)
    }

    pub fn beardifier() -> Self {
        owned(P::Beardifier)
    }

    pub fn end_outer_islands() -> Self {
        owned(P::EndOuterIslands)
    }

    pub fn distance_to_point(point: [i32; 3], metric: DistanceMetric) -> Self {
        owned(P::DistanceToPoint { point, metric })
    }

    pub fn shift_a(noise: &str) -> Self {
        owned(P::ShiftA {
            noise: NoiseHolder::Reference(id(noise)),
        })
    }

    pub fn shift_b(noise: &str) -> Self {
        owned(P::ShiftB {
            noise: NoiseHolder::Reference(id(noise)),
        })
    }

    pub fn noise(noise: &str, xz_scale: f64, y_scale: f64) -> Self {
        Self::shifted_noise(noise, xz_scale, y_scale, Self::ZERO, Self::ZERO)
    }

    pub fn shifted_noise_2d(shift_x: &Self, shift_z: &Self, xz_scale: f64, noise: &str) -> Self {
        Self::shifted_noise(noise, xz_scale, 0.0, shift_x.clone(), shift_z.clone())
    }

    fn shifted_noise(
        noise: &str,
        xz_scale: f64,
        y_scale: f64,
        shift_x: Self,
        shift_z: Self,
    ) -> Self {
        owned(P::Noise {
            noise: NoiseHolder::Reference(id(noise)),
            xz_scale: HashableF64(xz_scale),
            y_scale: HashableF64(y_scale),
            shift_x,
            shift_y: Self::ZERO,
            shift_z,
        })
    }

    /// A noise whose `[-1, 1]` is stretched onto `[min_target, max_target]`.
    pub fn mapped_noise(
        noise: &str,
        xz_scale: f64,
        y_scale: f64,
        min_target: f32,
        max_target: f32,
    ) -> Self {
        Self::noise(noise, xz_scale, y_scale).remap(-1.0, 1.0, min_target, max_target)
    }

    pub fn old_blended_noise(
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        owned(P::OldBlendedNoise(BlendedNoiseArguments {
            xz_scale: ScaleValue(xz_scale),
            y_scale: ScaleValue(y_scale),
            xz_factor: ScaleValue(xz_factor),
            y_factor: ScaleValue(y_factor),
            smear_scale_multiplier: SmearScaleMultiplier(smear_scale_multiplier),
        }))
    }

    pub fn y_gradient(from_y: i32, to_y: i32, from_value: f32, to_value: f32) -> Self {
        owned(P::Gradient(GradientArguments {
            axis: Axis::Y,
            tiling: Tiling::ClampToEdge,
            from_coordinate: from_y,
            to_coordinate: to_y,
            from_value: from_value.into(),
            to_value: to_value.into(),
        }))
    }

    /// The block height itself, over twice the height any dimension can have.
    pub fn y() -> Self {
        let (bottom, top) = (MIN_SURFACE_LOWER_BOUND, MAX_SURFACE_LOWER_BOUND);
        Self::y_gradient(bottom, top, bottom as f32, top as f32)
    }

    pub fn lerp(alpha: impl Into<Self>, first: impl Into<Self>, second: impl Into<Self>) -> Self {
        owned(P::Lerp {
            alpha: alpha.into(),
            first: first.into(),
            second: second.into(),
        })
    }

    /// An open end of `range` is the bound no noise value reaches.
    pub fn range_choice(
        input: impl Into<Self>,
        range: impl RangeBounds<f32>,
        when_in_range: impl Into<Self>,
        when_out_of_range: impl Into<Self>,
    ) -> Self {
        let bound = |bound: Bound<&f32>, open: f32| match bound {
            Bound::Included(value) | Bound::Excluded(value) => *value,
            Bound::Unbounded => open,
        };
        owned(P::RangeChoice {
            input: input.into(),
            min_inclusive: bound(range.start_bound(), -MAX_REASONABLE_NOISE_VALUE).into(),
            max_exclusive: bound(range.end_bound(), MAX_REASONABLE_NOISE_VALUE).into(),
            when_in_range: when_in_range.into(),
            when_out_of_range: when_out_of_range.into(),
        })
    }

    pub fn interval_select(
        input: impl Into<Self>,
        thresholds: &[f32],
        functions: Vec<Self>,
    ) -> Self {
        owned(P::IntervalSelect(IntervalSelectArguments {
            input: input.into(),
            thresholds: thresholds.iter().copied().map(NoiseValue::from).collect(),
            functions,
        }))
    }

    /// One noise sampled at a frequency `input` picks: each interval samples
    /// it `1 / rarity` as often and scales the result back up by `rarity`.
    pub fn rarity_select(
        input: impl Into<Self>,
        noise: &str,
        thresholds: &[f32],
        rarities: &[f32],
    ) -> Self {
        let functions = rarities
            .iter()
            .map(|rarity| {
                let scale = 1.0 / *rarity as f64;
                Self::noise(noise, scale, scale) * *rarity
            })
            .collect();
        Self::interval_select(input, thresholds, functions).abs()
    }

    pub fn find_top_surface(
        density: impl Into<Self>,
        upper_bound: impl Into<Self>,
        lower_bound: i32,
        cell_height: u32,
    ) -> Self {
        owned(P::FindTopSurface(FindTopSurfaceArguments {
            density: density.into(),
            upper_bound: upper_bound.into(),
            lower_bound,
            cell_height: NonZeroU32::new(cell_height).expect("a cell has a height"),
        }))
    }
}

/// A density function registry being filled: defining an entry hands back the
/// reference the entries after it name it by.
#[derive(Debug, Default)]
pub struct Functions(pub BTreeMap<ResourceLocation, Df>);

impl Functions {
    pub fn define(&mut self, name: &str, function: Df) -> Df {
        let replaced = self.0.insert(id(name), function);
        assert!(replaced.is_none(), "{name} is defined twice");
        Df::reference(name)
    }
}
