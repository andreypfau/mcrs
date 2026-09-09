pub mod args;
pub mod noise;
pub mod settings;
pub mod spline;

pub use args::{
    BlendedNoiseArguments, ClampArguments, FindTopSurfaceArguments, GradientArguments,
    IntervalSelectArguments, PowFunctionArguments, RoundFunctionArguments, ScaleValue,
    SingleArgumentFunction, SmearScaleMultiplier, TwoArgumentFunction,
};
pub use noise::{NoiseHolder, NoiseParam, Normalization};
pub use settings::{BlockState, Either, ValueRange};
pub use spline::{ProtoMultipoint, ProtoSpline};

use crate::node::distance::DistanceMetric;
use crate::volume::Axis;
use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;

/// Equality and hashing over the raw bits of a `f64` newtype, because vanilla
/// compares these records with `Double.compare`: `-0.0` and `0.0` are distinct
/// keys, and a derived `PartialEq` would call them equal while the bit hash
/// disagreed.
macro_rules! eq_by_bits {
    ($name:ident) => {
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.0.to_bits() == other.0.to_bits()
            }
        }

        impl Eq for $name {}

        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.to_bits().hash(state);
            }
        }
    };
}
pub(crate) use eq_by_bits;

/// A codec bound the field list alone does not express. The shape is derived as
/// usual and `validated!` hangs the check on the way in, so the fields are
/// spelled once rather than once more in a shadow struct that has to be kept in
/// step by hand.
pub(crate) trait Validate: Sized {
    fn validate(&self) -> Result<(), String>;
}

/// Turns the inherent codec `#[serde(remote = "Self")]` generates back into the
/// trait impls, checking [`Validate`] on the way in.
macro_rules! validated {
    ($($name:ident),* $(,)?) => {$(
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let value = $name::deserialize(deserializer)?;
                value.validate().map_err(serde::de::Error::custom)?;
                Ok(value)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                $name::serialize(self, serializer)
            }
        }
    )*};
}
pub(crate) use validated;

/// A `Codec.DOUBLE` payload.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HashableF64(pub f64);

eq_by_bits!(HashableF64);

impl From<f64> for HashableF64 {
    #[inline]
    fn from(value: f64) -> Self {
        HashableF64(value)
    }
}

pub const MAX_REASONABLE_NOISE_VALUE: f32 = 1_000_000.0;

/// `NOISE_VALUE_CODEC`. Stored as `f64` because that is what `serde_json`
/// parses, but bounded and compared as the `float` vanilla keeps.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(try_from = "f64")]
pub struct NoiseValue(pub f64);

impl PartialEq for NoiseValue {
    fn eq(&self, other: &Self) -> bool {
        (self.0 as f32).to_bits() == (other.0 as f32).to_bits()
    }
}

impl Eq for NoiseValue {}

impl Hash for NoiseValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.0 as f32).to_bits().hash(state);
    }
}

impl TryFrom<f64> for NoiseValue {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let narrowed = value as f32;
        if !(-MAX_REASONABLE_NOISE_VALUE..=MAX_REASONABLE_NOISE_VALUE).contains(&narrowed) {
            return Err(format!(
                "Value must be within range [-{MAX_REASONABLE_NOISE_VALUE};{MAX_REASONABLE_NOISE_VALUE}]: {value}"
            ));
        }
        Ok(NoiseValue(value))
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstantValue {
    pub value: NoiseValue,
}

impl From<f64> for ConstantValue {
    #[inline]
    fn from(value: f64) -> Self {
        ConstantValue {
            value: NoiseValue(value),
        }
    }
}

/// The tagged `minecraft:constant` carries `{"value": n}`, but a bare number is
/// the shape every shipped asset uses and the only one vanilla ever writes.
mod bare_value {
    use super::{ConstantValue, NoiseValue};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        value: &ConstantValue,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.value.0.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ConstantValue, D::Error> {
        NoiseValue::deserialize(deserializer).map(|value| ConstantValue { value })
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub enum DensityFunctionHolder {
    Value(#[serde(with = "bare_value")] ConstantValue),
    Reference(ResourceLocation),
    Owned(Box<ProtoDensityFunction>),
}

impl DensityFunctionHolder {
    /// The `instanceof ConstantFunction` test every compile-time specialization
    /// performs. A bare number and a tagged `constant` are the same function to
    /// vanilla, so both answer here.
    pub fn as_constant(&self) -> Option<f32> {
        match self {
            Self::Value(c) => Some(c.value.0 as f32),
            Self::Owned(f) => match &**f {
                ProtoDensityFunction::Constant(c) => Some(c.value.0 as f32),
                _ => None,
            },
            Self::Reference(_) => None,
        }
    }

    /// Structural equality against `DensityFunctions.zero()`, which is a record
    /// over a `float`: a shift of `-0.0` is *not* zero and forces the shifted
    /// noise sampler.
    pub fn is_zero_constant(&self) -> bool {
        self.as_constant().is_some_and(|v| v.to_bits() == 0)
    }
}

impl From<SingleArgumentFunction> for DensityFunctionHolder {
    fn from(function: SingleArgumentFunction) -> Self {
        function.input
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum ProtoDensityFunction {
    #[serde(rename = "minecraft:constant")]
    Constant(ConstantValue),
    #[serde(rename = "minecraft:blend_alpha")]
    BlendAlpha,
    #[serde(rename = "minecraft:blend_offset")]
    BlendOffset,
    #[serde(rename = "minecraft:beardifier")]
    Beardifier,
    #[serde(rename = "minecraft:noise")]
    Noise {
        noise: NoiseHolder,
        xz_scale: HashableF64,
        y_scale: HashableF64,
        #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
        shift_x: DensityFunctionHolder,
        #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
        shift_y: DensityFunctionHolder,
        #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
        shift_z: DensityFunctionHolder,
    },
    #[serde(rename = "minecraft:end_outer_islands")]
    EndOuterIslands,
    #[serde(rename = "minecraft:distance_to_point")]
    DistanceToPoint {
        point: [i32; 3],
        metric: DistanceMetric,
    },
    #[serde(rename = "minecraft:gradient")]
    Gradient(GradientArguments),
    #[serde(rename = "minecraft:shift_a")]
    ShiftA { noise: NoiseHolder },
    #[serde(rename = "minecraft:shift_b")]
    ShiftB { noise: NoiseHolder },
    #[serde(rename = "minecraft:shift")]
    Shift { noise: NoiseHolder },
    #[serde(rename = "minecraft:abs")]
    Abs(SingleArgumentFunction),
    #[serde(rename = "minecraft:square")]
    Square(SingleArgumentFunction),
    #[serde(rename = "minecraft:cube")]
    Cube(SingleArgumentFunction),
    #[serde(rename = "minecraft:sqrt")]
    Sqrt(SingleArgumentFunction),
    #[serde(rename = "minecraft:half_negative")]
    HalfNegative(SingleArgumentFunction),
    #[serde(rename = "minecraft:quarter_negative")]
    QuarterNegative(SingleArgumentFunction),
    #[serde(rename = "minecraft:reciprocal")]
    Reciprocal(SingleArgumentFunction),
    #[serde(rename = "minecraft:negate")]
    Negate(SingleArgumentFunction),
    #[serde(rename = "minecraft:squeeze")]
    Squeeze(SingleArgumentFunction),
    #[serde(rename = "minecraft:log")]
    Log(SingleArgumentFunction),
    #[serde(rename = "minecraft:sign")]
    Sign(SingleArgumentFunction),
    #[serde(rename = "minecraft:floor")]
    Floor(RoundFunctionArguments),
    #[serde(rename = "minecraft:round")]
    Round(RoundFunctionArguments),
    #[serde(rename = "minecraft:ceil")]
    Ceil(RoundFunctionArguments),
    #[serde(rename = "minecraft:truncate")]
    Truncate(RoundFunctionArguments),
    #[serde(rename = "minecraft:add")]
    Add(TwoArgumentFunction),
    #[serde(rename = "minecraft:sub")]
    Sub(TwoArgumentFunction),
    #[serde(rename = "minecraft:mul")]
    Mul(TwoArgumentFunction),
    #[serde(rename = "minecraft:div")]
    Div(TwoArgumentFunction),
    #[serde(rename = "minecraft:min")]
    Min(TwoArgumentFunction),
    #[serde(rename = "minecraft:max")]
    Max(TwoArgumentFunction),
    #[serde(rename = "minecraft:pow")]
    Pow(PowFunctionArguments),
    #[serde(rename = "minecraft:spline")]
    Spline { spline: ProtoSpline },
    #[serde(rename = "minecraft:lerp")]
    Lerp {
        alpha: DensityFunctionHolder,
        first: DensityFunctionHolder,
        second: DensityFunctionHolder,
    },
    #[serde(rename = "minecraft:clamp")]
    Clamp(ClampArguments),
    #[serde(rename = "minecraft:range_choice")]
    RangeChoice {
        input: DensityFunctionHolder,
        min_inclusive: NoiseValue,
        max_exclusive: NoiseValue,
        when_in_range: DensityFunctionHolder,
        when_out_of_range: DensityFunctionHolder,
    },
    #[serde(rename = "minecraft:interval_select")]
    IntervalSelect(IntervalSelectArguments),
    #[serde(rename = "minecraft:cache")]
    Cache(SingleArgumentFunction),
    #[serde(rename = "minecraft:blend_density")]
    BlendDensity(SingleArgumentFunction),
    #[serde(rename = "minecraft:interpolated")]
    Interpolated {
        input: DensityFunctionHolder,
        cell_size_xz: NonZeroU32,
        cell_size_y: NonZeroU32,
    },
    #[serde(rename = "minecraft:slice")]
    Slice {
        axis: Axis,
        coordinate: i32,
        input: DensityFunctionHolder,
    },
    #[serde(rename = "minecraft:find_top_surface")]
    FindTopSurface(FindTopSurfaceArguments),
    #[serde(rename = "minecraft:old_blended_noise")]
    OldBlendedNoise(BlendedNoiseArguments),
}

fn zero_holder() -> DensityFunctionHolder {
    DensityFunctionHolder::Value(ConstantValue::from(0.0))
}

fn is_zero_holder(shift: &DensityFunctionHolder) -> bool {
    shift.is_zero_constant()
}

impl ProtoDensityFunction {
    pub fn visit_children(&self, f: &mut impl FnMut(&DensityFunctionHolder)) {
        use ProtoDensityFunction::*;
        match self {
            Constant(_)
            | BlendAlpha
            | BlendOffset
            | Beardifier
            | EndOuterIslands
            | DistanceToPoint { .. }
            | Gradient(_)
            | ShiftA { .. }
            | ShiftB { .. }
            | Shift { .. }
            | OldBlendedNoise(_) => {}
            Abs(x) | Square(x) | Cube(x) | Sqrt(x) | HalfNegative(x) | QuarterNegative(x)
            | Reciprocal(x) | Negate(x) | Squeeze(x) | Log(x) | Sign(x) | Cache(x)
            | BlendDensity(x) => f(&x.input),
            Clamp(x) => f(&x.input),
            Interpolated { input, .. } | Slice { input, .. } => f(input),
            Noise {
                shift_x,
                shift_y,
                shift_z,
                ..
            } => {
                f(shift_x);
                f(shift_y);
                f(shift_z);
            }
            Floor(x) | Round(x) | Ceil(x) | Truncate(x) => {
                f(&x.input);
                f(&x.multiple);
            }
            Add(x) | Sub(x) | Mul(x) | Div(x) | Min(x) | Max(x) => {
                f(&x.left);
                f(&x.right);
            }
            Pow(x) => {
                f(&x.base);
                f(&x.exponent);
            }
            Lerp {
                alpha,
                first,
                second,
            } => {
                f(alpha);
                f(first);
                f(second);
            }
            Spline { spline } => spline.visit_coordinates(f),
            RangeChoice {
                input,
                when_in_range,
                when_out_of_range,
                ..
            } => {
                f(input);
                f(when_in_range);
                f(when_out_of_range);
            }
            IntervalSelect(x) => {
                f(&x.input);
                for function in &x.functions {
                    f(function);
                }
            }
            FindTopSurface(x) => {
                f(&x.density);
                f(&x.upper_bound);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_constant_round_trips_as_a_number() {
        let parsed: DensityFunctionHolder = serde_json::from_str("1.5").unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), "1.5");
        assert_eq!(parsed.as_constant(), Some(1.5));
    }

    #[test]
    fn a_tagged_constant_round_trips() {
        let parsed: ProtoDensityFunction =
            serde_json::from_str(r#"{"type":"minecraft:constant","value":1.5}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            r#"{"type":"minecraft:constant","value":1.5}"#
        );
    }

    /// The corpus always writes the namespace. A datapack that leaves it off is
    /// naming a kind this build does not have, not the `minecraft` one.
    #[test]
    fn a_type_without_a_namespace_is_a_load_error() {
        assert!(
            serde_json::from_str::<ProtoDensityFunction>(r#"{"type":"constant","value":1.5}"#)
                .is_err()
        );
    }

    /// The three-way `noise` specialization tests structural equality against
    /// `ConstantFunction(+0.0)`, and `Float.compare` separates the two zeros.
    #[test]
    fn a_negative_zero_shift_is_not_the_default() {
        let plain: ProtoDensityFunction = serde_json::from_str(
            r#"{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":1.0,"y_scale":1.0}"#,
        )
        .unwrap();
        let negative: ProtoDensityFunction = serde_json::from_str(
            r#"{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":1.0,"y_scale":1.0,"shift_x":-0.0}"#,
        )
        .unwrap();
        assert_ne!(plain, negative);

        let ProtoDensityFunction::Noise { shift_x, .. } = &negative else {
            unreachable!()
        };
        assert!(!shift_x.is_zero_constant());
        // A zero shift is the default, so it never reaches the encoded form.
        assert_eq!(
            serde_json::to_string(&plain).unwrap(),
            r#"{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":1.0,"y_scale":1.0}"#
        );
    }
}
