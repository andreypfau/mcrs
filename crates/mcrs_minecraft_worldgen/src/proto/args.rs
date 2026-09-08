use crate::node::gradient::Tiling;
use crate::proto::{ConstantValue, DensityFunctionHolder, NoiseValue};
use crate::volume::Axis;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SingleArgumentFunction {
    pub input: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoArgumentFunction {
    pub left: DensityFunctionHolder,
    pub right: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowFunctionArguments {
    pub base: DensityFunctionHolder,
    pub exponent: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoundFunctionArguments {
    pub input: DensityFunctionHolder,
    #[serde(
        default = "default_multiple",
        skip_serializing_if = "is_default_multiple"
    )]
    pub multiple: DensityFunctionHolder,
}

fn default_multiple() -> DensityFunctionHolder {
    DensityFunctionHolder::Value(ConstantValue::from(1.0))
}

/// `optionalFieldOf` drops the field on encode when it equals the default, and
/// the default is a `ConstantFunction(1.0f)` however the datapack spelled it.
fn is_default_multiple(multiple: &DensityFunctionHolder) -> bool {
    multiple
        .as_constant()
        .is_some_and(|v| v.to_bits() == 1.0f32.to_bits())
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedClamp")]
pub struct ClampArguments {
    pub input: DensityFunctionHolder,
    pub min: NoiseValue,
    pub max: NoiseValue,
}

#[derive(Deserialize)]
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

fn is_clamp_to_edge(tiling: &Tiling) -> bool {
    matches!(tiling, Tiling::ClampToEdge)
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedGradient")]
pub struct GradientArguments {
    pub axis: Axis,
    #[serde(default, skip_serializing_if = "is_clamp_to_edge")]
    pub tiling: Tiling,
    pub from_coordinate: i32,
    pub to_coordinate: i32,
    pub from_value: NoiseValue,
    pub to_value: NoiseValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedGradient {
    axis: Axis,
    #[serde(default)]
    tiling: Tiling,
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

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedIntervalSelect")]
pub struct IntervalSelectArguments {
    pub input: DensityFunctionHolder,
    pub thresholds: Vec<NoiseValue>,
    pub functions: Vec<DensityFunctionHolder>,
}

#[derive(Deserialize)]
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
        // `Comparators.isInOrder(thresholds, Float::compare)`, which orders -0.0
        // below 0.0. A plain `<` on the payload would accept `[0.0, -0.0]`.
        let out_of_order = raw
            .thresholds
            .windows(2)
            .any(|w| (w[0].0 as f32).total_cmp(&(w[1].0 as f32)).is_gt());
        if out_of_order {
            return Err("Threshold values must be ordered from smallest to largest".to_string());
        }
        Ok(IntervalSelectArguments {
            input: raw.input,
            thresholds: raw.thresholds,
            functions: raw.functions,
        })
    }
}

pub const MIN_SURFACE_LOWER_BOUND: i32 = -4064;
pub const MAX_SURFACE_LOWER_BOUND: i32 = 4062;

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedFindTopSurface")]
pub struct FindTopSurfaceArguments {
    pub density: DensityFunctionHolder,
    pub upper_bound: DensityFunctionHolder,
    pub lower_bound: i32,
    pub cell_height: NonZeroU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedFindTopSurface {
    density: DensityFunctionHolder,
    upper_bound: DensityFunctionHolder,
    lower_bound: i32,
    cell_height: NonZeroU32,
}

impl TryFrom<UncheckedFindTopSurface> for FindTopSurfaceArguments {
    type Error = String;

    fn try_from(raw: UncheckedFindTopSurface) -> Result<Self, Self::Error> {
        if !(MIN_SURFACE_LOWER_BOUND..=MAX_SURFACE_LOWER_BOUND).contains(&raw.lower_bound) {
            return Err(format!(
                "Value must be within range [{MIN_SURFACE_LOWER_BOUND};{MAX_SURFACE_LOWER_BOUND}]: {}",
                raw.lower_bound
            ));
        }
        Ok(FindTopSurfaceArguments {
            density: raw.density,
            upper_bound: raw.upper_bound,
            lower_bound: raw.lower_bound,
            cell_height: raw.cell_height,
        })
    }
}

pub const MIN_BLENDED_NOISE_SCALE: f64 = 0.001;
pub const MAX_BLENDED_NOISE_SCALE: f64 = 1000.0;
pub const MIN_SMEAR_SCALE_MULTIPLIER: f64 = 1.0;
pub const MAX_SMEAR_SCALE_MULTIPLIER: f64 = 8.0;

/// The only density function whose scale fields carry codec bounds. All five
/// stay `f64`: narrowing them is exact for the four shipped instantiations and
/// for nothing else.
#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlendedNoiseArguments {
    pub xz_scale: ScaleValue,
    pub y_scale: ScaleValue,
    pub xz_factor: ScaleValue,
    pub y_factor: ScaleValue,
    pub smear_scale_multiplier: SmearScaleMultiplier,
}

macro_rules! bounded_f64 {
    ($name:ident, $min:expr, $max:expr) => {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
        #[serde(try_from = "f64")]
        pub struct $name(pub f64);

        crate::proto::eq_by_bits!($name);

        impl TryFrom<f64> for $name {
            type Error = String;

            fn try_from(value: f64) -> Result<Self, Self::Error> {
                if !($min..=$max).contains(&value) {
                    return Err(format!(
                        "Value must be within range [{};{}]: {value}",
                        $min, $max
                    ));
                }
                Ok($name(value))
            }
        }
    };
}

bounded_f64!(ScaleValue, MIN_BLENDED_NOISE_SCALE, MAX_BLENDED_NOISE_SCALE);
bounded_f64!(
    SmearScaleMultiplier,
    MIN_SMEAR_SCALE_MULTIPLIER,
    MAX_SMEAR_SCALE_MULTIPLIER
);

#[cfg(test)]
mod tests {
    use crate::proto::ProtoDensityFunction;

    fn rejects(json: &str) -> String {
        match serde_json::from_str::<ProtoDensityFunction>(json) {
            Ok(parsed) => panic!("{json} must not parse, but gave {parsed:?}"),
            Err(e) => e.to_string(),
        }
    }

    /// `Float::compare` puts -0.0 below 0.0, so one order loads and its reverse
    /// does not.
    #[test]
    fn interval_select_orders_the_two_zeros() {
        serde_json::from_str::<ProtoDensityFunction>(
            r#"{"type":"minecraft:interval_select","input":0.0,"thresholds":[-0.0,0.0],"functions":[1.0,2.0,3.0]}"#,
        )
        .unwrap();
        assert!(
            rejects(
                r#"{"type":"minecraft:interval_select","input":0.0,"thresholds":[0.0,-0.0],"functions":[1.0,2.0,3.0]}"#
            )
            .contains("ordered")
        );
    }

    #[test]
    fn a_round_omits_a_multiple_of_one_however_it_was_spelled() {
        let tagged: ProtoDensityFunction = serde_json::from_str(
            r#"{"type":"minecraft:floor","input":0.5,"multiple":{"type":"minecraft:constant","value":1.0}}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&tagged).unwrap(),
            r#"{"type":"minecraft:floor","input":0.5}"#
        );
    }

    #[test]
    fn blended_noise_scales_are_bounded() {
        let ok = r#"{"type":"minecraft:old_blended_noise","xz_scale":0.25,"y_scale":0.125,"xz_factor":80.0,"y_factor":160.0,"smear_scale_multiplier":8.0}"#;
        serde_json::from_str::<ProtoDensityFunction>(ok).unwrap();
        rejects(&ok.replace(
            "\"smear_scale_multiplier\":8.0",
            "\"smear_scale_multiplier\":9.0",
        ));
        rejects(&ok.replace("\"xz_scale\":0.25", "\"xz_scale\":0.0"));
    }

    #[test]
    fn find_top_surface_bounds_its_lower_bound() {
        let ok = r#"{"type":"minecraft:find_top_surface","density":0.0,"upper_bound":1.0,"lower_bound":-4064,"cell_height":8}"#;
        serde_json::from_str::<ProtoDensityFunction>(ok).unwrap();
        rejects(&ok.replace("-4064", "-4065"));
        rejects(&ok.replace("\"cell_height\":8", "\"cell_height\":0"));
    }
}
