pub mod args;
pub mod build;
pub mod settings;
pub mod spline;

pub use args::{
    BlendedNoiseArguments, ClampArguments, FindTopSurfaceArguments, GradientArguments,
    IntervalSelectArguments, PowFunctionArguments, RoundFunctionArguments, ScaleValue,
    SingleArgumentFunction, SmearScaleMultiplier, TwoArgumentFunction,
};
pub use settings::{Either, ValueRange};
pub use spline::{ProtoMultipoint, ProtoSpline, SplinePoints};

use crate::node::distance::DistanceMetric;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_noise::proto::{HashableF64, NoiseHolder};
use mcrs_minecraft_worldgen_noise::sample_grid::Axis;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;

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

/// The game writes a `float` as its shortest decimal, which is not the `f64`
/// nearest its bits: `0.1f32 as f64` is `0.10000000149011612`.
impl From<f32> for NoiseValue {
    fn from(value: f32) -> Self {
        NoiseValue(
            value
                .to_string()
                .parse()
                .expect("a float prints as a float"),
        )
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
#[serde(remote = "Self")]
#[serde(deny_unknown_fields)]
pub enum ProtoDensityFunction {
    Constant(ConstantValue),
    BlendAlpha,
    BlendOffset,
    Beardifier,
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
    EndOuterIslands,
    DistanceToPoint {
        point: [i32; 3],
        metric: DistanceMetric,
    },
    Gradient(GradientArguments),
    ShiftA {
        noise: NoiseHolder,
    },
    ShiftB {
        noise: NoiseHolder,
    },
    Shift {
        noise: NoiseHolder,
    },
    Abs(SingleArgumentFunction),
    Square(SingleArgumentFunction),
    Cube(SingleArgumentFunction),
    Sqrt(SingleArgumentFunction),
    HalfNegative(SingleArgumentFunction),
    QuarterNegative(SingleArgumentFunction),
    Reciprocal(SingleArgumentFunction),
    Negate(SingleArgumentFunction),
    Squeeze(SingleArgumentFunction),
    Log(SingleArgumentFunction),
    Sign(SingleArgumentFunction),
    Floor(RoundFunctionArguments),
    Round(RoundFunctionArguments),
    Ceil(RoundFunctionArguments),
    Truncate(RoundFunctionArguments),
    Add(TwoArgumentFunction),
    Sub(TwoArgumentFunction),
    Mul(TwoArgumentFunction),
    Div(TwoArgumentFunction),
    Min(TwoArgumentFunction),
    Max(TwoArgumentFunction),
    Pow(PowFunctionArguments),
    Spline {
        spline: ProtoSpline,
    },
    Lerp {
        alpha: DensityFunctionHolder,
        first: DensityFunctionHolder,
        second: DensityFunctionHolder,
    },
    Clamp(ClampArguments),
    RangeChoice {
        input: DensityFunctionHolder,
        min_inclusive: NoiseValue,
        max_exclusive: NoiseValue,
        when_in_range: DensityFunctionHolder,
        when_out_of_range: DensityFunctionHolder,
    },
    IntervalSelect(IntervalSelectArguments),
    Cache(SingleArgumentFunction),
    BlendDensity(SingleArgumentFunction),
    Interpolated {
        input: DensityFunctionHolder,
        cell_size_xz: NonZeroU32,
        cell_size_y: NonZeroU32,
    },
    Slice {
        axis: Axis,
        coordinate: i32,
        input: DensityFunctionHolder,
    },
    FindTopSurface(FindTopSurfaceArguments),
    OldBlendedNoise(BlendedNoiseArguments),
}

mcrs_minecraft_registry::dispatch! {
    ProtoDensityFunction, key = "type", registry = crate::keys::DensityFunctionType,
    {
        Constant => Constant,
        BlendAlpha => BlendAlpha,
        BlendOffset => BlendOffset,
        Beardifier => Beardifier,
        Noise => Noise,
        EndOuterIslands => EndOuterIslands,
        DistanceToPoint => DistanceToPoint,
        Gradient => Gradient,
        ShiftA => ShiftA,
        ShiftB => ShiftB,
        Shift => Shift,
        Abs => Abs,
        Square => Square,
        Cube => Cube,
        Sqrt => Sqrt,
        HalfNegative => HalfNegative,
        QuarterNegative => QuarterNegative,
        Reciprocal => Reciprocal,
        Negate => Negate,
        Squeeze => Squeeze,
        Log => Log,
        Sign => Sign,
        Floor => Floor,
        Round => Round,
        Ceil => Ceil,
        Truncate => Truncate,
        Add => Add,
        Sub => Sub,
        Mul => Mul,
        Div => Div,
        Min => Min,
        Max => Max,
        Pow => Pow,
        Spline => Spline,
        Lerp => Lerp,
        Clamp => Clamp,
        RangeChoice => RangeChoice,
        IntervalSelect => IntervalSelect,
        Cache => Cache,
        BlendDensity => BlendDensity,
        Interpolated => Interpolated,
        Slice => Slice,
        FindTopSurface => FindTopSurface,
        OldBlendedNoise => OldBlendedNoise,
    }
}

fn zero_holder() -> DensityFunctionHolder {
    DensityFunctionHolder::ZERO
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
    use crate::router::NoiseGeneratorSettings;
    use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
    use mcrs_minecraft_worldgen_testing::round_trips;

    #[test]
    fn every_noise_round_trips() {
        assert_eq!(round_trips::<NoiseParam>("noise"), 69);
    }

    #[test]
    fn every_density_function_round_trips() {
        assert_eq!(round_trips::<DensityFunctionHolder>("density_function"), 65);
    }

    #[test]
    fn every_noise_settings_round_trips() {
        assert_eq!(round_trips::<NoiseGeneratorSettings>("noise_settings"), 8);
    }

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

    /// A type without a namespace is the `minecraft` one, as the game reads
    /// it; one that names nothing is a load error.
    #[test]
    fn a_type_without_a_namespace_is_a_load_error() {
        assert!(
            serde_json::from_str::<ProtoDensityFunction>(r#"{"type":"constant","value":1.5}"#)
                .is_ok()
        );
        let error =
            serde_json::from_str::<ProtoDensityFunction>(r#"{"type":"nonsense","value":1.5}"#)
                .unwrap_err()
                .to_string();
        assert!(error.contains("minecraft:nonsense"), "{error}");
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
