use crate::interval::Interval;
use crate::proto::HashableF64;
use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Serialize};

/// `RegistryCodecs.holder(Registries.NOISE, …)` with inlining allowed: an id
/// into `worldgen/noise`, or the parameters object itself.
#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NoiseHolder {
    Reference(ResourceLocation),
    Owned(NoiseParam),
}

pub const MIN_BASE_OCTAVE: i32 = -32;
pub const MAX_BASE_OCTAVE: i32 = 32;
pub const MIN_OCTAVE_COUNT: usize = 1;
pub const MAX_OCTAVE_COUNT: usize = 32;
pub const MAX_AMPLITUDE: f64 = 1_000_000.0;
/// `Codec.doubleRange(1.0E-5F, …)`: a float literal widened, so the bound is
/// `1.0000000474974513e-5` and not the decimal it looks like.
pub const MIN_BASE_AMPLITUDE: f64 = 1.0e-5f32 as f64;

const TARGET_DEVIATION: f64 = 0.3333333333333333;
const PERLIN_STANDARD_DEVIATION: f64 = 0.2702247831245211;

/// `NormalNoise.Parameters`: the datapack shape alone. Everything derived from
/// it — the surviving octaves, their weights and the declared range — belongs to
/// [`crate::noise::normal::NormalNoise`].
#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "UncheckedNoiseParam")]
pub struct NoiseParam {
    pub base_octave: i32,
    #[serde(default = "default_base_amplitude", skip_serializing_if = "is_one")]
    pub base_amplitude: HashableF64,
    #[serde(
        default = "default_octave_count",
        skip_serializing_if = "is_one_octave"
    )]
    pub octave_count: usize,
    #[serde(default, skip_serializing_if = "Normalization::is_default")]
    pub normalize: Normalization,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amplitude_modifiers: Vec<HashableF64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedNoiseParam {
    base_octave: i32,
    #[serde(default = "default_base_amplitude")]
    base_amplitude: HashableF64,
    #[serde(default = "default_octave_count")]
    octave_count: usize,
    #[serde(default)]
    normalize: Normalization,
    #[serde(default)]
    amplitude_modifiers: Vec<HashableF64>,
}

fn default_base_amplitude() -> HashableF64 {
    HashableF64(1.0)
}

fn default_octave_count() -> usize {
    1
}

fn is_one(amplitude: &HashableF64) -> bool {
    amplitude.0.to_bits() == 1.0f64.to_bits()
}

fn is_one_octave(count: &usize) -> bool {
    *count == 1
}

impl TryFrom<UncheckedNoiseParam> for NoiseParam {
    type Error = String;

    fn try_from(raw: UncheckedNoiseParam) -> Result<Self, Self::Error> {
        if !(MIN_BASE_OCTAVE..=MAX_BASE_OCTAVE).contains(&raw.base_octave) {
            return Err(format!(
                "Value must be within range [{MIN_BASE_OCTAVE};{MAX_BASE_OCTAVE}]: {}",
                raw.base_octave
            ));
        }
        if !(MIN_BASE_AMPLITUDE..=MAX_AMPLITUDE).contains(&raw.base_amplitude.0) {
            return Err(format!(
                "Value must be within range [{MIN_BASE_AMPLITUDE};{MAX_AMPLITUDE}]: {}",
                raw.base_amplitude.0
            ));
        }
        if !(MIN_OCTAVE_COUNT..=MAX_OCTAVE_COUNT).contains(&raw.octave_count) {
            return Err(format!(
                "Value must be within range [{MIN_OCTAVE_COUNT};{MAX_OCTAVE_COUNT}]: {}",
                raw.octave_count
            ));
        }
        if raw.amplitude_modifiers.len() > MAX_OCTAVE_COUNT {
            return Err(format!(
                "List must have at most {MAX_OCTAVE_COUNT} elements: {}",
                raw.amplitude_modifiers.len()
            ));
        }
        if let Some(bad) = raw
            .amplitude_modifiers
            .iter()
            .find(|m| !(0.0..=MAX_AMPLITUDE).contains(&m.0))
        {
            return Err(format!(
                "Value must be within range [0.0;{MAX_AMPLITUDE}]: {}",
                bad.0
            ));
        }
        if !raw.amplitude_modifiers.is_empty() && raw.amplitude_modifiers.len() != raw.octave_count
        {
            return Err(format!(
                "amplitude_modifiers had size {}, but octave_count was {}",
                raw.amplitude_modifiers.len(),
                raw.octave_count
            ));
        }
        Ok(NoiseParam {
            base_octave: raw.base_octave,
            base_amplitude: raw.base_amplitude,
            octave_count: raw.octave_count,
            normalize: raw.normalize,
            amplitude_modifiers: raw.amplitude_modifiers,
        })
    }
}

impl NoiseParam {
    /// `getAmplitudeModifier`: an empty list means every octave is unmodified.
    /// A non-empty one is validated at load to have exactly `octave_count`
    /// entries, so indexing can never miss.
    pub fn octave_amplitudes(&self) -> Vec<f64> {
        (0..self.octave_count)
            .map(|i| self.amplitude_modifiers.get(i).map(|m| m.0).unwrap_or(1.0))
            .collect()
    }

    /// Everything the parameters decide before a seed is drawn: which octaves
    /// survive, what each weighs, and the factor that scales them all. Building
    /// the sampler reads this rather than deriving it a second time.
    pub fn octaves(&self) -> Octaves {
        let modifiers = self.octave_amplitudes();
        let count = modifiers.len() as i32;
        let base_amplitude = self.base_amplitude.0;
        let mut amplitude = match self.normalize {
            Normalization::Disabled => base_amplitude,
            _ => base_amplitude * (2.0f64.powi(count - 1) / (2.0f64.powi(count) - 1.0)),
        };

        // Octaves with a zero modifier are dropped, not zeroed: they contribute
        // to neither the amplitude sum nor the deviation.
        let mut amplitudes = Vec::with_capacity(modifiers.len());
        for modifier in &modifiers {
            amplitudes.push((*modifier != 0.0).then_some(amplitude * *modifier));
            amplitude *= 0.5;
        }

        let mut target_amplitude = compensated_sum(amplitudes.iter().flatten().map(|a| a.abs()));
        let input_deviation = deviation(amplitudes.iter().flatten().copied());
        let mut factor = if input_deviation == 0.0 {
            0.0
        } else {
            (target_amplitude * TARGET_DEVIATION) / (input_deviation * std::f64::consts::SQRT_2)
        };

        if self.normalize == Normalization::Legacy && factor != 0.0 {
            let lowest = modifiers.iter().position(|m| *m != 0.0).unwrap();
            let highest = modifiers.iter().rposition(|m| *m != 0.0).unwrap();
            let parity = parity_normalization_factor(base_amplitude, (highest - lowest) as f64);
            target_amplitude *= parity / factor;
            factor = parity;
        }

        Octaves {
            amplitudes,
            factor,
            target_amplitude,
        }
    }

}

/// What [`NoiseParam::octaves`] decides before any seed is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Octaves {
    /// One entry per declared octave; `None` where the modifier was zero and the
    /// octave is dropped rather than weighted to nothing.
    pub amplitudes: Vec<Option<f64>>,
    /// Scales every layer so the summed octaves reach the target deviation.
    pub factor: f64,
    /// The summed absolute octave amplitudes, after the legacy adjustment.
    pub target_amplitude: f64,
}

/// Six sigma on the summed octaves, deliberately not the analytically rigorous
/// extreme — that is about twice as wide. Branch elimination consumes this, so
/// widening it silently changes generated terrain.
pub(crate) fn declared_range(target_amplitude: f64) -> Interval {
    Interval::symmetric((target_amplitude * TARGET_DEVIATION * 6.0) as f32)
}

pub(crate) fn deviation(amplitudes: impl Iterator<Item = f64>) -> f64 {
    let mut variance = 0.0f64;
    for a in amplitudes {
        let layer_deviation = PERLIN_STANDARD_DEVIATION * a.abs();
        variance += layer_deviation * layer_deviation;
    }
    variance.sqrt()
}

pub(crate) fn parity_normalization_factor(base_amplitude: f64, octave_span: f64) -> f64 {
    let expected_deviation = 0.1 * (1.0 + 1.0 / (octave_span + 1.0));
    base_amplitude * 0.5 * TARGET_DEVIATION / expected_deviation
}

/// `DoubleStream.sum` is Kahan-compensated and falls back to the naive total
/// when compensation produces a NaN from infinite inputs.
pub(crate) fn compensated_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0f64;
    let mut compensation = 0.0f64;
    let mut simple = 0.0f64;
    for value in values {
        let corrected = value - compensation;
        let next = sum + corrected;
        compensation = (next - sum) - corrected;
        sum = next;
        simple += value;
    }
    let total = sum - compensation;
    if total.is_nan() && simple.is_infinite() {
        simple
    } else {
        total
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy, Default)]
pub enum Normalization {
    Disabled,
    #[default]
    Enabled,
    Legacy,
}

impl Normalization {
    fn is_default(&self) -> bool {
        matches!(self, Normalization::Enabled)
    }
}

impl Serialize for Normalization {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Normalization::Disabled => serializer.serialize_bool(false),
            Normalization::Enabled => serializer.serialize_bool(true),
            Normalization::Legacy => serializer.serialize_str("legacy"),
        }
    }
}

impl<'de> Deserialize<'de> for Normalization {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noise::normal::NormalNoise;

    fn param(json: &str) -> NoiseParam {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn normalization_round_trips_all_three_shapes() {
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
            let parsed = param(json);
            assert_eq!(parsed.normalize, expected);
            let reencoded = serde_json::to_string(&parsed).unwrap();
            assert_eq!(param(&reencoded).normalize, expected);
        }
        assert_eq!(
            param(r#"{"base_octave":-7}"#).normalize,
            Normalization::Enabled
        );
        serde_json::from_str::<NoiseParam>(r#"{"base_octave":-7,"normalize":"nope"}"#).unwrap_err();
    }

    #[test]
    fn amplitude_modifiers_must_match_the_octave_count() {
        param(r#"{"base_octave":-7,"octave_count":2,"amplitude_modifiers":[1.0,0.5]}"#);
        param(r#"{"base_octave":-7,"octave_count":2}"#);
        serde_json::from_str::<NoiseParam>(
            r#"{"base_octave":-7,"octave_count":3,"amplitude_modifiers":[1.0,0.5]}"#,
        )
        .unwrap_err();
        serde_json::from_str::<NoiseParam>(
            r#"{"base_octave":-7,"octave_count":1,"amplitude_modifiers":[-1.0]}"#,
        )
        .unwrap_err();
    }

    /// A zero modifier drops its octave from both sums rather than contributing
    /// zero, which is what makes the legacy span `highest - lowest` and not
    /// `octave_count - 1`.
    #[test]
    fn a_gapped_octave_narrows_the_range() {
        let gapped = param(
            r#"{"base_octave":-9,"octave_count":3,"amplitude_modifiers":[1.0,0.0,1.0],"normalize":"legacy"}"#,
        );
        let solid = param(
            r#"{"base_octave":-9,"octave_count":3,"amplitude_modifiers":[1.0,1.0,1.0],"normalize":"legacy"}"#,
        );
        let range = |p: &NoiseParam| NormalNoise::new(p.clone()).range().max();
        assert!(range(&gapped) < range(&solid));
        assert!(range(&gapped) > 0.0);
    }

    #[test]
    fn a_disabled_normalization_keeps_the_base_amplitude() {
        let disabled = param(r#"{"base_octave":-3,"octave_count":1,"normalize":false}"#);
        assert_eq!(
            NormalNoise::new(disabled).range().max(),
            (1.0f64 * 0.3333333333333333 * 6.0) as f32
        );
    }
}
