use mcrs_minecraft_core::ResourceLocation;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HashableF64(pub f64);

// Normally this is bad, but we just care about checking if components are the same
impl Hash for HashableF64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_le_bytes().hash(state);
    }
}

impl Eq for HashableF64 {}

impl From<f64> for HashableF64 {
    #[inline]
    fn from(value: f64) -> Self {
        HashableF64(value)
    }
}

pub const MAX_REASONABLE_NOISE_VALUE: f64 = 1_000_000.0;

/// A density value carried by a datapack field, range-checked at load.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "f64")]
pub struct NoiseValue(pub f64);

impl Hash for NoiseValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_le_bytes().hash(state);
    }
}

impl Eq for NoiseValue {}

impl TryFrom<f64> for NoiseValue {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_nan() || value.abs() > MAX_REASONABLE_NOISE_VALUE {
            return Err(format!(
                "Value must be within range [-{MAX_REASONABLE_NOISE_VALUE};{MAX_REASONABLE_NOISE_VALUE}]: {value}"
            ));
        }
        Ok(NoiseValue(value))
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
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

#[derive(Hash, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub enum DensityFunctionHolder {
    Value(#[serde(with = "bare_value")] ConstantValue),
    Reference(ResourceLocation),
    Owned(Box<ProtoDensityFunction>),
}

impl From<SingleArgumentFunction> for DensityFunctionHolder {
    fn from(func: SingleArgumentFunction) -> Self {
        func.input
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ProtoDensityFunction {
    #[serde(alias = "minecraft:blend_alpha")]
    BlendAlpha,
    #[serde(alias = "minecraft:blend_offset")]
    BlendOffset,
    #[serde(alias = "minecraft:beardifier")]
    Beardifier,
    #[serde(alias = "old_blended_noise", rename = "minecraft:old_blended_noise")]
    OldBlendedNoise {
        xz_scale: HashableF64,
        y_scale: HashableF64,
        xz_factor: HashableF64,
        y_factor: HashableF64,
        smear_scale_multiplier: HashableF64,
    },
    #[serde(alias = "interpolated", rename = "minecraft:interpolated")]
    Interpolated {
        input: DensityFunctionHolder,
        cell_size_xz: std::num::NonZeroU32,
        cell_size_y: std::num::NonZeroU32,
    },
    #[serde(alias = "minecraft:cache")]
    Cache(SingleArgumentFunction),
    #[serde(alias = "noise", rename = "minecraft:noise")]
    Noise {
        noise: NoiseHolder,
        xz_scale: HashableF64,
        y_scale: HashableF64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shift_x: Option<DensityFunctionHolder>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shift_y: Option<DensityFunctionHolder>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shift_z: Option<DensityFunctionHolder>,
    },
    #[serde(alias = "minecraft:end_outer_islands")]
    EndOuterIslands,
    #[serde(rename = "minecraft:range_choice")]
    RangeChoice {
        input: DensityFunctionHolder,
        min_inclusive: NoiseValue,
        max_exclusive: NoiseValue,
        when_in_range: DensityFunctionHolder,
        when_out_of_range: DensityFunctionHolder,
    },
    #[serde(alias = "minecraft:shift_a")]
    ShiftA { noise: NoiseHolder },
    #[serde(alias = "minecraft:shift_b")]
    ShiftB { noise: NoiseHolder },
    #[serde(rename = "minecraft:shift", alias = "shift")]
    Shift { noise: NoiseHolder },
    #[serde(rename = "minecraft:blend_density", alias = "blend_density")]
    BlendDensity(SingleArgumentFunction),
    #[serde(alias = "minecraft:clamp")]
    Clamp(ClampArguments),
    #[serde(alias = "minecraft:abs")]
    Abs(SingleArgumentFunction),
    #[serde(alias = "minecraft:square")]
    Square(SingleArgumentFunction),
    #[serde(alias = "minecraft:cube")]
    Cube(SingleArgumentFunction),
    #[serde(alias = "minecraft:half_negative")]
    HalfNegative(SingleArgumentFunction),
    #[serde(alias = "minecraft:quarter_negative")]
    QuarterNegative(SingleArgumentFunction),
    #[serde(alias = "minecraft:reciprocal")]
    Reciprocal(SingleArgumentFunction),
    #[serde(alias = "minecraft:negate")]
    Negate(SingleArgumentFunction),
    #[serde(alias = "minecraft:squeeze")]
    Squeeze(SingleArgumentFunction),
    #[serde(alias = "minecraft:sqrt")]
    Sqrt(SingleArgumentFunction),
    #[serde(alias = "minecraft:log")]
    Log(SingleArgumentFunction),
    #[serde(alias = "minecraft:sign")]
    Sign(SingleArgumentFunction),
    #[serde(alias = "minecraft:pow")]
    Pow(PowFunctionArguments),
    #[serde(alias = "minecraft:floor")]
    Floor(RoundFunctionArguments),
    #[serde(alias = "minecraft:round")]
    Round(RoundFunctionArguments),
    #[serde(alias = "minecraft:ceil")]
    Ceil(RoundFunctionArguments),
    #[serde(alias = "minecraft:truncate")]
    Truncate(RoundFunctionArguments),
    #[serde(alias = "minecraft:add")]
    Add(TwoArgumentFunction),
    #[serde(alias = "minecraft:mul")]
    Mul(TwoArgumentFunction),
    #[serde(alias = "minecraft:sub")]
    Sub(TwoArgumentFunction),
    #[serde(alias = "minecraft:div")]
    Div(TwoArgumentFunction),
    #[serde(alias = "minecraft:min")]
    Min(TwoArgumentFunction),
    #[serde(alias = "minecraft:max")]
    Max(TwoArgumentFunction),
    #[serde(alias = "minecraft:spline")]
    Spline { spline: SplineHolder },
    #[serde(alias = "minecraft:constant")]
    Constant(ConstantValue),
    #[serde(alias = "minecraft:gradient")]
    Gradient(GradientArguments),
    #[serde(alias = "minecraft:lerp")]
    Lerp {
        alpha: DensityFunctionHolder,
        first: DensityFunctionHolder,
        second: DensityFunctionHolder,
    },
    #[serde(alias = "minecraft:slice")]
    Slice {
        axis: Axis,
        coordinate: i32,
        input: DensityFunctionHolder,
    },
    #[serde(alias = "minecraft:interval_select")]
    IntervalSelect(IntervalSelectArguments),
    #[serde(alias = "minecraft:distance_to_point")]
    DistanceToPoint {
        point: [i32; 3],
        metric: DistanceMetric,
    },
    #[serde(alias = "minecraft:find_top_surface")]
    FindTopSurface {
        density: DensityFunctionHolder,
        upper_bound: DensityFunctionHolder,
        lower_bound: i32,
        cell_height: std::num::NonZeroU32,
    },
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedClamp")]
pub struct ClampArguments {
    pub input: DensityFunctionHolder,
    pub min: NoiseValue,
    pub max: NoiseValue,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedClamp {
    input: DensityFunctionHolder,
    min: NoiseValue,
    max: NoiseValue,
}

impl TryFrom<UncheckedClamp> for ClampArguments {
    type Error = String;

    fn try_from(raw: UncheckedClamp) -> Result<Self, Self::Error> {
        if raw.max.0 < raw.min.0 {
            return Err(format!(
                "min ({}) must be less than or equal to max ({})",
                raw.min.0, raw.max.0
            ));
        }
        Ok(ClampArguments {
            input: raw.input,
            min: raw.min,
            max: raw.max,
        })
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedGradient")]
pub struct GradientArguments {
    pub axis: Axis,
    #[serde(default)]
    pub tiling: TilingMode,
    pub from_coordinate: i32,
    pub to_coordinate: i32,
    pub from_value: NoiseValue,
    pub to_value: NoiseValue,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedGradient {
    axis: Axis,
    #[serde(default)]
    tiling: TilingMode,
    from_coordinate: i32,
    to_coordinate: i32,
    from_value: NoiseValue,
    to_value: NoiseValue,
}

impl TryFrom<UncheckedGradient> for GradientArguments {
    type Error = String;

    fn try_from(raw: UncheckedGradient) -> Result<Self, Self::Error> {
        if raw.from_coordinate == raw.to_coordinate {
            return Err("from_coordinate cannot be equal to to_coordinate".to_string());
        }
        Ok(GradientArguments {
            axis: raw.axis,
            tiling: raw.tiling,
            from_coordinate: raw.from_coordinate,
            to_coordinate: raw.to_coordinate,
            from_value: raw.from_value,
            to_value: raw.to_value,
        })
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedIntervalSelect")]
pub struct IntervalSelectArguments {
    pub input: DensityFunctionHolder,
    pub thresholds: Vec<NoiseValue>,
    pub functions: Vec<DensityFunctionHolder>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedIntervalSelect {
    input: DensityFunctionHolder,
    thresholds: Vec<NoiseValue>,
    functions: Vec<DensityFunctionHolder>,
}

impl TryFrom<UncheckedIntervalSelect> for IntervalSelectArguments {
    type Error = String;

    fn try_from(raw: UncheckedIntervalSelect) -> Result<Self, Self::Error> {
        if raw.functions.len() < 2 {
            return Err(format!(
                "List must have at least 2 elements: {}",
                raw.functions.len()
            ));
        }
        if raw.thresholds.len() != raw.functions.len() - 1 {
            return Err(format!(
                "Expected {} thresholds for {} functions, but got {}",
                raw.functions.len() - 1,
                raw.functions.len(),
                raw.thresholds.len()
            ));
        }
        if raw.thresholds.windows(2).any(|w| w[1].0 < w[0].0) {
            return Err("Threshold values must be ordered from smallest to largest".to_string());
        }
        Ok(IntervalSelectArguments {
            input: raw.input,
            thresholds: raw.thresholds,
            functions: raw.functions,
        })
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum NoiseHolder {
    Reference(ResourceLocation),
    Owned(NoiseParam),
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct NoiseParam {
    pub base_octave: i32,
    #[cfg_attr(
        feature = "serde",
        serde(default = "NoiseParam::default_base_amplitude")
    )]
    pub base_amplitude: HashableF64,
    #[cfg_attr(feature = "serde", serde(default = "NoiseParam::default_octave_count"))]
    pub octave_count: usize,
    #[cfg_attr(feature = "serde", serde(default))]
    pub normalize: Normalization,
    #[cfg_attr(feature = "serde", serde(default))]
    pub amplitude_modifiers: Vec<HashableF64>,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy, Default)]
pub enum Normalization {
    Disabled,
    #[default]
    Enabled,
    Legacy,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Normalization {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Normalization::Disabled => serializer.serialize_bool(false),
            Normalization::Enabled => serializer.serialize_bool(true),
            Normalization::Legacy => serializer.serialize_str("legacy"),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Normalization {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NormalizationVisitor;

        impl serde::de::Visitor<'_> for NormalizationVisitor {
            type Value = Normalization;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a boolean or the string \"legacy\"")
            }

            fn visit_bool<E: serde::de::Error>(self, enabled: bool) -> Result<Normalization, E> {
                Ok(if enabled {
                    Normalization::Enabled
                } else {
                    Normalization::Disabled
                })
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Normalization, E> {
                match value {
                    "legacy" => Ok(Normalization::Legacy),
                    other => Err(E::custom(format!("Invalid normalization type: {other}"))),
                }
            }
        }

        deserializer.deserialize_any(NormalizationVisitor)
    }
}

impl NoiseParam {
    fn default_base_amplitude() -> HashableF64 {
        HashableF64(1.0)
    }

    fn default_octave_count() -> usize {
        1
    }

    pub fn octave_amplitudes(&self) -> Vec<f64> {
        (0..self.octave_count)
            .map(|i| self.amplitude_modifiers.get(i).map(|m| m.0).unwrap_or(1.0))
            .collect()
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum TilingMode {
    #[default]
    ClampToEdge,
    Repeat,
    MirroredRepeat,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DistanceMetric {
    Euclidean,
    EuclideanSquared,
    Manhattan,
    Chebyshev,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct SingleArgumentFunction {
    pub input: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
pub enum RoundingMode {
    Floor,
    Round,
    Ceil,
    Truncate,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct PowFunctionArguments {
    pub base: DensityFunctionHolder,
    pub exponent: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct RoundFunctionArguments {
    pub input: DensityFunctionHolder,
    #[cfg_attr(
        feature = "serde",
        serde(default = "RoundFunctionArguments::default_multiple")
    )]
    pub multiple: DensityFunctionHolder,
}

impl RoundFunctionArguments {
    fn default_multiple() -> DensityFunctionHolder {
        DensityFunctionHolder::Value(ConstantValue::from(1.0))
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct TwoArgumentFunction {
    pub left: DensityFunctionHolder,
    pub right: DensityFunctionHolder,
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[serde(untagged)]
pub enum SplineHolder {
    Constant(HashableF64),
    Spline(Spline),
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(deny_unknown_fields, try_from = "UncheckedSpline")
)]
pub struct Spline {
    pub coordinate: DensityFunctionHolder,
    pub points: Vec<SplinePoint>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedSpline {
    coordinate: DensityFunctionHolder,
    points: Vec<SplinePoint>,
}

impl TryFrom<UncheckedSpline> for Spline {
    type Error = String;

    fn try_from(raw: UncheckedSpline) -> Result<Self, Self::Error> {
        if raw.points.is_empty() {
            return Err("List must have contents".to_string());
        }
        Ok(Spline {
            coordinate: raw.coordinate,
            points: raw.points,
        })
    }
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct SplinePoint {
    pub location: HashableF64,
    pub value: SplineHolder,
    pub derivative: HashableF64,
}

pub trait Visitor {
    fn visit_density_function_holder(&mut self, function: &DensityFunctionHolder) {
        match function {
            DensityFunctionHolder::Value(v) => self.visit_constant(v.value.0),
            DensityFunctionHolder::Reference(r) => self.visit_reference(r),
            DensityFunctionHolder::Owned(f) => self.visit_density_function(f),
        }
    }

    fn visit_constant(&mut self, value: f64) {}

    fn visit_reference(&mut self, value: &ResourceLocation) {}

    fn visit_noise_holder(&mut self, noise: &NoiseHolder) {}

    fn visit_density_function(&mut self, function: &ProtoDensityFunction) {
        match function {
            ProtoDensityFunction::BlendAlpha => self.visit_blend_alpha(),
            ProtoDensityFunction::BlendOffset => self.visit_blend_offset(),
            ProtoDensityFunction::Beardifier => self.visit_beardifier(),
            ProtoDensityFunction::OldBlendedNoise {
                xz_scale,
                y_scale,
                xz_factor,
                y_factor,
                smear_scale_multiplier,
            } => self.visit_old_blended_noise(
                xz_scale.0,
                y_scale.0,
                xz_factor.0,
                y_factor.0,
                smear_scale_multiplier.0,
            ),
            ProtoDensityFunction::Interpolated {
                input,
                cell_size_xz,
                cell_size_y,
            } => self.visit_interpolated(input, *cell_size_xz, *cell_size_y),
            ProtoDensityFunction::Cache(arg) => self.visit_cache(arg),
            ProtoDensityFunction::Noise {
                noise,
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
            } => self.visit_noise(
                noise,
                xz_scale.0,
                y_scale.0,
                shift_x.as_ref(),
                shift_y.as_ref(),
                shift_z.as_ref(),
            ),
            ProtoDensityFunction::EndOuterIslands => self.visit_end_outer_islands(),
            ProtoDensityFunction::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => self.visit_range_choice(
                input,
                min_inclusive.0,
                max_exclusive.0,
                when_in_range,
                when_out_of_range,
            ),
            ProtoDensityFunction::ShiftA { noise } => self.visit_shift_a(noise),
            ProtoDensityFunction::ShiftB { noise } => self.visit_shift_b(noise),
            ProtoDensityFunction::Shift { noise } => self.visit_shift(noise),
            ProtoDensityFunction::BlendDensity(x) => self.visit_blend_density(x),
            ProtoDensityFunction::Clamp(x) => self.visit_clamp(&x.input, x.min.0, x.max.0),
            ProtoDensityFunction::Abs(x) => self.visit_abs(x),
            ProtoDensityFunction::Square(x) => self.visit_square(x),
            ProtoDensityFunction::Cube(x) => self.visit_cube(x),
            ProtoDensityFunction::HalfNegative(x) => self.visit_half_negative(x),
            ProtoDensityFunction::QuarterNegative(x) => self.visit_quarter_negative(x),
            ProtoDensityFunction::Reciprocal(x) => self.visit_reciprocal(x),
            ProtoDensityFunction::Negate(x) => self.visit_negate(x),
            ProtoDensityFunction::Squeeze(x) => self.visit_squeeze(x),
            ProtoDensityFunction::Sqrt(x) => self.visit_sqrt(x),
            ProtoDensityFunction::Log(x) => self.visit_log(x),
            ProtoDensityFunction::Sign(x) => self.visit_sign(x),
            ProtoDensityFunction::Pow(x) => self.visit_pow(x),
            ProtoDensityFunction::Floor(x) => self.visit_round(RoundingMode::Floor, x),
            ProtoDensityFunction::Round(x) => self.visit_round(RoundingMode::Round, x),
            ProtoDensityFunction::Ceil(x) => self.visit_round(RoundingMode::Ceil, x),
            ProtoDensityFunction::Truncate(x) => self.visit_round(RoundingMode::Truncate, x),
            ProtoDensityFunction::Add(x) => self.visit_add(x),
            ProtoDensityFunction::Mul(x) => self.visit_mul(x),
            ProtoDensityFunction::Sub(x) => self.visit_sub(x),
            ProtoDensityFunction::Div(x) => self.visit_div(x),
            ProtoDensityFunction::Min(x) => self.visit_min(x),
            ProtoDensityFunction::Max(x) => self.visit_max(x),
            ProtoDensityFunction::Spline { spline } => self.visit_spline(spline),
            ProtoDensityFunction::Constant(x) => self.visit_constant(x.value.0),
            ProtoDensityFunction::Gradient(x) => self.visit_gradient(
                x.axis,
                x.tiling,
                x.from_coordinate,
                x.to_coordinate,
                x.from_value.0,
                x.to_value.0,
            ),
            ProtoDensityFunction::Lerp {
                alpha,
                first,
                second,
            } => self.visit_lerp(alpha, first, second),
            ProtoDensityFunction::Slice {
                axis,
                coordinate,
                input,
            } => self.visit_slice(*axis, *coordinate, input),
            ProtoDensityFunction::IntervalSelect(x) => {
                self.visit_interval_select(&x.input, &x.thresholds, &x.functions)
            }
            ProtoDensityFunction::DistanceToPoint { point, metric } => {
                self.visit_distance_to_point(*point, *metric)
            }
            ProtoDensityFunction::FindTopSurface {
                density,
                upper_bound,
                lower_bound,
                cell_height,
            } => self.visit_find_top_surface(density, upper_bound, *lower_bound, *cell_height),
        }
    }

    fn visit_blend_alpha(&mut self) {}
    fn visit_blend_offset(&mut self) {}
    fn visit_beardifier(&mut self) {}
    fn visit_old_blended_noise(
        &mut self,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) {
        // No inner functions to visit
    }

    fn visit_single_argument_function(&mut self, function: &SingleArgumentFunction) {
        self.visit_density_function_holder(&function.input)
    }

    fn visit_two_argument_function(&mut self, function: &TwoArgumentFunction) {
        self.visit_density_function_holder(&function.left);
        self.visit_density_function_holder(&function.right)
    }

    fn visit_interpolated(
        &mut self,
        input: &DensityFunctionHolder,
        cell_size_xz: std::num::NonZeroU32,
        cell_size_y: std::num::NonZeroU32,
    ) {
        self.visit_density_function_holder(input)
    }

    fn visit_cache(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }

    fn visit_noise(
        &mut self,
        noise: &NoiseHolder,
        xz_scale: f64,
        y_scale: f64,
        shift_x: Option<&DensityFunctionHolder>,
        shift_y: Option<&DensityFunctionHolder>,
        shift_z: Option<&DensityFunctionHolder>,
    ) {
        for shift in [shift_x, shift_y, shift_z].into_iter().flatten() {
            self.visit_density_function_holder(shift);
        }
        self.visit_noise_holder(noise)
    }

    fn visit_end_outer_islands(&mut self) {}

    fn visit_range_choice(
        &mut self,
        input: &DensityFunctionHolder,
        min_inclusive: f64,
        max_exclusive: f64,
        when_in_range: &DensityFunctionHolder,
        when_out_of_range: &DensityFunctionHolder,
    ) {
        self.visit_density_function_holder(input);
        self.visit_density_function_holder(when_in_range);
        self.visit_density_function_holder(when_out_of_range)
    }

    fn visit_shift_a(&mut self, function: &NoiseHolder) {
        self.visit_noise_holder(function)
    }
    fn visit_shift_b(&mut self, function: &NoiseHolder) {
        self.visit_noise_holder(function)
    }
    fn visit_shift(&mut self, argument: &NoiseHolder) {
        self.visit_noise_holder(argument)
    }
    fn visit_blend_density(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_clamp(&mut self, input: &DensityFunctionHolder, min: f64, max: f64) {
        self.visit_density_function_holder(input)
    }
    fn visit_abs(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_square(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_cube(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_half_negative(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_quarter_negative(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_reciprocal(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_negate(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_squeeze(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_sqrt(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_log(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_sign(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_pow(&mut self, function: &PowFunctionArguments) {
        self.visit_density_function_holder(&function.base);
        self.visit_density_function_holder(&function.exponent)
    }
    fn visit_round(&mut self, mode: RoundingMode, function: &RoundFunctionArguments) {
        self.visit_density_function_holder(&function.input);
        self.visit_density_function_holder(&function.multiple)
    }
    fn visit_add(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_mul(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_sub(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_div(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_min(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_max(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_spline(&mut self, spline: &SplineHolder) {
        match spline {
            SplineHolder::Constant(v) => self.visit_constant(v.0),
            SplineHolder::Spline(spline) => {
                self.visit_density_function_holder(&spline.coordinate);
                for point in &spline.points {
                    self.visit_constant(point.location.0);
                    self.visit_spline(&point.value);
                    self.visit_constant(point.derivative.0);
                }
            }
        }
    }
    fn visit_gradient(
        &mut self,
        axis: Axis,
        tiling: TilingMode,
        from_coordinate: i32,
        to_coordinate: i32,
        from_value: f64,
        to_value: f64,
    ) {
    }

    fn visit_lerp(
        &mut self,
        alpha: &DensityFunctionHolder,
        first: &DensityFunctionHolder,
        second: &DensityFunctionHolder,
    ) {
        self.visit_density_function_holder(alpha);
        self.visit_density_function_holder(first);
        self.visit_density_function_holder(second)
    }

    fn visit_slice(&mut self, axis: Axis, coordinate: i32, input: &DensityFunctionHolder) {
        self.visit_density_function_holder(input)
    }

    fn visit_interval_select(
        &mut self,
        input: &DensityFunctionHolder,
        thresholds: &[NoiseValue],
        functions: &[DensityFunctionHolder],
    ) {
        self.visit_density_function_holder(input);
        for function in functions {
            self.visit_density_function_holder(function);
        }
    }

    fn visit_distance_to_point(&mut self, point: [i32; 3], metric: DistanceMetric) {}
    fn visit_find_top_surface(
        &mut self,
        density: &DensityFunctionHolder,
        upper_bound: &DensityFunctionHolder,
        lower_bound: i32,
        cell_height: std::num::NonZeroU32,
    ) {
        self.visit_density_function_holder(density);
        self.visit_density_function_holder(upper_bound);
    }
}

pub const AXIS_X: u8 = 1;
pub const AXIS_Y: u8 = 2;
pub const AXIS_Z: u8 = 4;
pub const ALL_AXES: u8 = AXIS_X | AXIS_Y | AXIS_Z;

impl Axis {
    pub fn bit(self) -> u8 {
        match self {
            Axis::X => AXIS_X,
            Axis::Y => AXIS_Y,
            Axis::Z => AXIS_Z,
        }
    }
}

pub fn noise_scale_axes(xz_scale: f64, y_scale: f64) -> u8 {
    let mut axes = ALL_AXES;
    if y_scale == 0.0 {
        axes &= !AXIS_Y;
    }
    if xz_scale == 0.0 {
        axes &= !(AXIS_X | AXIS_Z);
    }
    axes
}

impl SplineHolder {
    pub fn visit_children(&self, f: &mut impl FnMut(&DensityFunctionHolder)) {
        if let SplineHolder::Spline(spline) = self {
            f(&spline.coordinate);
            for point in &spline.points {
                point.value.visit_children(f);
            }
        }
    }

    pub fn visit_children_mut(&mut self, f: &mut impl FnMut(&mut DensityFunctionHolder)) {
        if let SplineHolder::Spline(spline) = self {
            f(&mut spline.coordinate);
            for point in &mut spline.points {
                point.value.visit_children_mut(f);
            }
        }
    }
}

impl ProtoDensityFunction {
    pub fn visit_children(&self, f: &mut impl FnMut(&DensityFunctionHolder)) {
        use ProtoDensityFunction::*;
        match self {
            BlendAlpha
            | BlendOffset
            | Beardifier
            | OldBlendedNoise { .. }
            | EndOuterIslands
            | ShiftA { .. }
            | ShiftB { .. }
            | Shift { .. }
            | Constant(_)
            | Gradient(_)
            | DistanceToPoint { .. } => {}
            Interpolated { input, .. } | Slice { input, .. } => f(input),
            Clamp(x) => f(&x.input),
            Cache(x) | BlendDensity(x) | Abs(x) | Square(x) | Cube(x) | HalfNegative(x)
            | QuarterNegative(x) | Reciprocal(x) | Negate(x) | Squeeze(x) | Sqrt(x) | Log(x)
            | Sign(x) => f(&x.input),
            Noise {
                shift_x,
                shift_y,
                shift_z,
                ..
            } => {
                for shift in [shift_x, shift_y, shift_z].into_iter().flatten() {
                    f(shift);
                }
            }
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
            Pow(x) => {
                f(&x.base);
                f(&x.exponent);
            }
            Floor(x) | Round(x) | Ceil(x) | Truncate(x) => {
                f(&x.input);
                f(&x.multiple);
            }
            Add(x) | Mul(x) | Sub(x) | Div(x) | Min(x) | Max(x) => {
                f(&x.left);
                f(&x.right);
            }
            Spline { spline } => spline.visit_children(f),
            Lerp {
                alpha,
                first,
                second,
            } => {
                f(alpha);
                f(first);
                f(second);
            }
            IntervalSelect(x) => {
                f(&x.input);
                for function in &x.functions {
                    f(function);
                }
            }
            FindTopSurface {
                density,
                upper_bound,
                ..
            } => {
                f(density);
                f(upper_bound);
            }
        }
    }

    pub fn visit_children_mut(&mut self, f: &mut impl FnMut(&mut DensityFunctionHolder)) {
        use ProtoDensityFunction::*;
        match self {
            BlendAlpha
            | BlendOffset
            | Beardifier
            | OldBlendedNoise { .. }
            | EndOuterIslands
            | ShiftA { .. }
            | ShiftB { .. }
            | Shift { .. }
            | Constant(_)
            | Gradient(_)
            | DistanceToPoint { .. } => {}
            Interpolated { input, .. } | Slice { input, .. } => f(input),
            Clamp(x) => f(&mut x.input),
            Cache(x) | BlendDensity(x) | Abs(x) | Square(x) | Cube(x) | HalfNegative(x)
            | QuarterNegative(x) | Reciprocal(x) | Negate(x) | Squeeze(x) | Sqrt(x) | Log(x)
            | Sign(x) => f(&mut x.input),
            Noise {
                shift_x,
                shift_y,
                shift_z,
                ..
            } => {
                for shift in [shift_x, shift_y, shift_z].into_iter().flatten() {
                    f(shift);
                }
            }
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
            Pow(x) => {
                f(&mut x.base);
                f(&mut x.exponent);
            }
            Floor(x) | Round(x) | Ceil(x) | Truncate(x) => {
                f(&mut x.input);
                f(&mut x.multiple);
            }
            Add(x) | Mul(x) | Sub(x) | Div(x) | Min(x) | Max(x) => {
                f(&mut x.left);
                f(&mut x.right);
            }
            Spline { spline } => spline.visit_children_mut(f),
            Lerp {
                alpha,
                first,
                second,
            } => {
                f(alpha);
                f(first);
                f(second);
            }
            IntervalSelect(x) => {
                f(&mut x.input);
                for function in &mut x.functions {
                    f(function);
                }
            }
            FindTopSurface {
                density,
                upper_bound,
                ..
            } => {
                f(density);
                f(upper_bound);
            }
        }
    }

    pub fn domain_axes(&self) -> u8 {
        use ProtoDensityFunction::*;
        let children = || {
            let mut axes = 0u8;
            self.visit_children(&mut |child| axes |= child.domain_axes());
            axes
        };
        match self {
            BlendAlpha | BlendOffset | Beardifier | Constant(_) => 0,
            OldBlendedNoise { .. } | Shift { .. } | DistanceToPoint { .. } => ALL_AXES,
            ShiftA { .. } | ShiftB { .. } | EndOuterIslands => AXIS_X | AXIS_Z,
            Gradient(x) => x.axis.bit(),
            Noise {
                xz_scale, y_scale, ..
            } => children() | noise_scale_axes(xz_scale.0, y_scale.0),
            Slice { axis, .. } => children() & !axis.bit(),
            FindTopSurface { .. } => children() & !AXIS_Y,
            _ => children(),
        }
    }

    pub fn rewrite_children(&self, rule: &dyn RewriteRule) -> ProtoDensityFunction {
        let mut rewritten = self.clone();
        rewritten.visit_children_mut(&mut |child| *child = rule.rewrite(child));
        rewritten
    }
}

impl DensityFunctionHolder {
    pub fn domain_axes(&self) -> u8 {
        match self {
            DensityFunctionHolder::Value(_) => 0,
            // Inlining resolves every reference, so one that reaches here is opaque.
            // The mask is a may-vary over-approximation, so all axes stays sound.
            DensityFunctionHolder::Reference(_) => ALL_AXES,
            DensityFunctionHolder::Owned(function) => function.domain_axes(),
        }
    }

    pub fn rewrite_children(&self, rule: &dyn RewriteRule) -> DensityFunctionHolder {
        match self {
            DensityFunctionHolder::Value(_) | DensityFunctionHolder::Reference(_) => self.clone(),
            DensityFunctionHolder::Owned(function) => {
                DensityFunctionHolder::Owned(Box::new(function.rewrite_children(rule)))
            }
        }
    }
}

pub trait RewriteRule {
    fn rewrite(&self, function: &DensityFunctionHolder) -> DensityFunctionHolder;
}

pub struct InlineReference<'a>(
    pub &'a std::collections::BTreeMap<ResourceLocation, ProtoDensityFunction>,
);

impl RewriteRule for InlineReference<'_> {
    fn rewrite(&self, function: &DensityFunctionHolder) -> DensityFunctionHolder {
        match function {
            DensityFunctionHolder::Reference(id) => match self.0.get(id) {
                Some(resolved) => {
                    DensityFunctionHolder::Owned(Box::new(resolved.rewrite_children(self)))
                }
                None => function.clone(),
            },
            _ => function.rewrite_children(self),
        }
    }
}

/// Pin every axis a child cannot vary along but its parent can, so that the
/// child's whole subtree is known to be constant along it without any consumer
/// having to declare a cache.
pub struct SliceUniformAxes {
    parent_axes: u8,
}

impl SliceUniformAxes {
    pub fn new(parent_axes: u8) -> Self {
        Self { parent_axes }
    }
}

pub fn is_uniform_axis_slice_leaf(function: &DensityFunctionHolder) -> bool {
    match function {
        DensityFunctionHolder::Value(_) => true,
        DensityFunctionHolder::Owned(f) => matches!(
            **f,
            ProtoDensityFunction::Constant(_) | ProtoDensityFunction::Gradient(_)
        ),
        DensityFunctionHolder::Reference(_) => false,
    }
}

pub fn sliced_axes(function: &DensityFunctionHolder) -> u8 {
    let mut axes = 0u8;
    let mut current = function;
    while let DensityFunctionHolder::Owned(f) = current {
        let ProtoDensityFunction::Slice { axis, input, .. } = &**f else {
            break;
        };
        axes |= axis.bit();
        current = input;
    }
    axes
}

fn slice_at_origin(function: DensityFunctionHolder, axes: u8) -> DensityFunctionHolder {
    let axes = axes & !sliced_axes(&function);
    let mut function = function;
    for axis in [Axis::X, Axis::Z, Axis::Y] {
        if axes & axis.bit() != 0 {
            function = DensityFunctionHolder::Owned(Box::new(ProtoDensityFunction::Slice {
                axis,
                coordinate: 0,
                input: function,
            }));
        }
    }
    function
}

impl RewriteRule for SliceUniformAxes {
    fn rewrite(&self, function: &DensityFunctionHolder) -> DensityFunctionHolder {
        if is_uniform_axis_slice_leaf(function) {
            return function.clone();
        }
        let axes = function.domain_axes();
        if axes == self.parent_axes {
            return function.rewrite_children(self);
        }
        let rewritten = function.rewrite_children(&SliceUniformAxes { parent_axes: axes });
        slice_at_origin(rewritten, self.parent_axes & !axes)
    }
}

#[cfg(all(test, feature = "serde"))]
mod validation_tests {
    use super::{DensityFunctionHolder, NoiseParam, Normalization, ProtoDensityFunction};

    fn rejects(json: &str) -> String {
        match serde_json::from_str::<ProtoDensityFunction>(json) {
            Ok(parsed) => panic!("{json} must not parse, but gave {parsed:?}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn a_tagged_constant_round_trips_as_an_object() {
        let parsed: ProtoDensityFunction =
            serde_json::from_str(r#"{"type":"minecraft:constant","value":1.5}"#).unwrap();
        let reencoded = serde_json::to_string(&parsed).unwrap();
        assert_eq!(
            serde_json::from_str::<ProtoDensityFunction>(&reencoded).unwrap(),
            parsed
        );
    }

    #[test]
    fn a_bare_constant_round_trips_as_a_number() {
        let parsed: DensityFunctionHolder = serde_json::from_str("1.5").unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), "1.5");
    }

    #[test]
    fn a_clamp_cannot_invert_its_bounds() {
        assert!(
            rejects(r#"{"type":"clamp","input":0.0,"min":1.0,"max":-1.0}"#).contains("less than")
        );
    }

    #[test]
    fn a_clamp_cannot_leave_the_reasonable_noise_range() {
        rejects(r#"{"type":"clamp","input":0.0,"min":-2000000.0,"max":1.0}"#);
        rejects(r#"{"type":"clamp","input":0.0,"min":-1.0,"max":2000000.0}"#);
    }

    #[test]
    fn a_gradient_cannot_span_a_single_coordinate() {
        assert!(
            rejects(
                r#"{"type":"gradient","axis":"y","from_coordinate":4,"to_coordinate":4,"from_value":0.0,"to_value":1.0}"#
            )
            .contains("to_coordinate")
        );
    }

    #[test]
    fn find_top_surface_cannot_take_a_zero_cell_height() {
        rejects(
            r#"{"type":"find_top_surface","density":0.0,"upper_bound":1.0,"lower_bound":0,"cell_height":0}"#,
        );
    }

    #[test]
    fn interpolated_cannot_take_a_zero_cell_size() {
        rejects(r#"{"type":"interpolated","input":0.0,"cell_size_xz":0,"cell_size_y":8}"#);
        rejects(r#"{"type":"interpolated","input":0.0,"cell_size_xz":4,"cell_size_y":0}"#);
    }

    #[test]
    fn a_spline_cannot_be_empty() {
        rejects(r#"{"type":"spline","spline":{"coordinate":0.0,"points":[]}}"#);
    }

    #[test]
    fn interval_select_thresholds_must_match_the_branches() {
        assert!(
            rejects(
                r#"{"type":"interval_select","input":0.0,"thresholds":[0.0,1.0],"functions":[1.0,2.0]}"#
            )
            .contains("thresholds")
        );
        rejects(r#"{"type":"interval_select","input":0.0,"thresholds":[],"functions":[1.0]}"#);
    }

    #[test]
    fn interval_select_thresholds_must_increase() {
        assert!(
            rejects(
                r#"{"type":"interval_select","input":0.0,"thresholds":[1.0,0.0],"functions":[1.0,2.0,3.0]}"#
            )
            .contains("ordered")
        );
    }

    #[test]
    fn noise_normalization_round_trips_all_three_shapes() {
        for (json, expected) in [
            (
                r#"{"base_octave":-7,"normalize":false}"#,
                Normalization::Disabled,
            ),
            (
                r#"{"base_octave":-7,"normalize":true}"#,
                Normalization::Enabled,
            ),
            (
                r#"{"base_octave":-7,"normalize":"legacy"}"#,
                Normalization::Legacy,
            ),
        ] {
            let parsed: NoiseParam = serde_json::from_str(json).unwrap();
            assert_eq!(parsed.normalize, expected);
            let reencoded = serde_json::to_string(&parsed).unwrap();
            assert_eq!(
                serde_json::from_str::<NoiseParam>(&reencoded)
                    .unwrap()
                    .normalize,
                expected
            );
        }
        assert_eq!(
            serde_json::from_str::<NoiseParam>(r#"{"base_octave":-7}"#)
                .unwrap()
                .normalize,
            Normalization::Enabled
        );
        serde_json::from_str::<NoiseParam>(r#"{"base_octave":-7,"normalize":"nope"}"#).unwrap_err();
    }
}
