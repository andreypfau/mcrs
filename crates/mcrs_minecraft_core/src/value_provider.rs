use crate::codec::{Bounded, NonNegativeInt, is_default};
use mcrs_minecraft_random::Random;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// Where a vertical anchor sits, given the dimension's own extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightContext {
    pub min_y: i32,
    pub depth: i32,
    pub sea_level: i32,
}

impl HeightContext {
    /// `Level.isOutsideBuildHeight`, negated.
    #[inline]
    pub fn contains(self, y: i32) -> bool {
        y >= self.min_y && y < self.min_y + self.depth
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
    RelativeToSeaLevel(i32),
}

impl VerticalAnchor {
    pub fn resolve_y(self, context: HeightContext) -> i32 {
        match self {
            VerticalAnchor::Absolute(y) => y,
            VerticalAnchor::AboveBottom(offset) => context.min_y + offset,
            VerticalAnchor::BelowTop(offset) => context.min_y + context.depth - 1 - offset,
            VerticalAnchor::RelativeToSeaLevel(offset) => context.sea_level + offset,
        }
    }
}

/// One entry of a `WeightedList`: the value under `data`, its share under `weight`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weighted<T> {
    pub data: T,
    pub weight: NonNegativeInt,
}

/// `Mth.nextInt`, which — unlike `randomBetweenInclusive` — spends no draw on a
/// degenerate range. The height providers rely on that to keep their draw counts.
fn next_int_guarded<R: Random>(rng: &mut R, min: i32, max_inclusive: i32) -> i32 {
    if min >= max_inclusive {
        min
    } else {
        rng.next_int_between_inclusive(min, max_inclusive)
    }
}

fn normal<R: Random>(rng: &mut R, mean: f32, deviation: f32) -> f32 {
    mean + rng.next_gaussian() as f32 * deviation
}

fn non_empty_distribution<'de, D, T>(deserializer: D) -> Result<Vec<Weighted<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let distribution = Vec::<Weighted<T>>::deserialize(deserializer)?;
    if distribution.iter().map(|entry| entry.weight.0).sum::<i32>() <= 0 {
        return Err(D::Error::custom(
            "weighted list must contain at least one entry with non-zero weight",
        ));
    }
    Ok(distribution)
}

fn pick_weighted<'a, T, R: Random>(distribution: &'a [Weighted<T>], rng: &mut R) -> &'a T {
    &pick_weighted_by(distribution, |entry| entry.weight.0, rng)
        .expect("a non-empty distribution always covers its own total weight")
        .data
}

/// One `nextInt(total)` draw walked through the weights in list order; a total
/// of zero draws nothing.
pub fn pick_weighted_by<'a, T, R: Random>(
    items: &'a [T],
    weight: impl Fn(&T) -> i32,
    rng: &mut R,
) -> Option<&'a T> {
    let total: i32 = items.iter().map(&weight).sum();
    if total <= 0 {
        return None;
    }
    let mut selection = rng.next_i32_bound(total);
    items.iter().find(|item| {
        selection -= weight(item);
        selection < 0
    })
}

/// The explicit `constant` member of each provider registry. `Codec.either` reads a
/// bare value first and falls through to the dispatch, where `constant` names a map
/// codec over a single `value` field.
#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum ExplicitConstant<V> {
    #[serde(rename = "minecraft:constant")]
    Constant { value: V },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ConstantOrDispatch<V, D> {
    Bare(V),
    Explicit(ExplicitConstant<V>),
    Dispatched(D),
}

impl<V, D> ConstantOrDispatch<V, D> {
    /// A constant is always written back as the bare value — `IntProviders.CODEC`
    /// encodes through `Either.left(constantInt.value())` — so the two input forms
    /// collapse into one representation here rather than being remembered.
    fn split(self) -> Result<V, D> {
        match self {
            Self::Bare(value) | Self::Explicit(ExplicitConstant::Constant { value }) => Ok(value),
            Self::Dispatched(dispatched) => Err(dispatched),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum IntProvider {
    /// A bare number, which `Codec.either` reads as the constant form.
    Constant(i32),
    Dispatched(DispatchedIntProvider),
}

impl<'de> Deserialize<'de> for IntProvider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ConstantOrDispatch::<i32, DispatchedIntProvider>::deserialize(deserializer)?.split() {
            Ok(value) => Ok(Self::Constant(value)),
            Err(dispatched) => {
                span_ordered(dispatched.span()).map_err(D::Error::custom)?;
                Ok(Self::Dispatched(dispatched))
            }
        }
    }
}

/// The range checks the reference's codecs make: a span whose max falls below
/// its min, or a plateau wider than the span, is a load error.
fn span_ordered<T: PartialOrd + std::ops::Sub<Output = T> + Copy + std::fmt::Display>(
    span: Option<(T, T, Option<T>)>,
) -> Result<(), String> {
    let Some((min, max, plateau)) = span else {
        return Ok(());
    };
    if max < min {
        return Err(format!("Max must be at least min: [{min}, {max}]"));
    }
    if plateau.is_some_and(|plateau| plateau > max - min) {
        return Err(format!(
            "Plateau can at most be the full span: [{min}, {max}]"
        ));
    }
    Ok(())
}

impl DispatchedIntProvider {
    /// The `(min, max, plateau)` the variant is bounded by, for the ones that are.
    fn span(&self) -> Option<(i32, i32, Option<i32>)> {
        use DispatchedIntProvider::*;
        match self {
            Uniform {
                min_inclusive,
                max_inclusive,
            }
            | BiasedToBottom {
                min_inclusive,
                max_inclusive,
            }
            | Clamped {
                min_inclusive,
                max_inclusive,
                ..
            }
            | ClampedNormal {
                min_inclusive,
                max_inclusive,
                ..
            } => Some((*min_inclusive, *max_inclusive, None)),
            Trapezoid { min, max, plateau } => Some((*min, *max, Some(*plateau))),
            VeryBiasedToBottom { .. } | WeightedList { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum DispatchedIntProvider {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:biased_to_bottom")]
    BiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:very_biased_to_bottom")]
    VeryBiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:clamped")]
    Clamped {
        source: Box<IntProvider>,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:weighted_list")]
    WeightedList {
        #[serde(deserialize_with = "non_empty_distribution")]
        distribution: Vec<Weighted<IntProvider>>,
    },
    #[serde(rename = "minecraft:clamped_normal")]
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid { min: i32, max: i32, plateau: i32 },
}

/// `IntProviders.codec(min, max)`: a provider whose whole range must sit inside
/// `[MIN, MAX]`, refused at load the way the reference's codec refuses it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BoundedIntProvider<const MIN: i32, const MAX: i32>(pub IntProvider);

impl<const MIN: i32, const MAX: i32> std::ops::Deref for BoundedIntProvider<MIN, MAX> {
    type Target = IntProvider;

    fn deref(&self) -> &IntProvider {
        &self.0
    }
}

impl<'de, const MIN: i32, const MAX: i32> Deserialize<'de> for BoundedIntProvider<MIN, MAX> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let provider = IntProvider::deserialize(deserializer)?;
        let (low, high) = provider.bounds();
        if low < MIN {
            return Err(D::Error::custom(format!(
                "Value provider too low: {MIN} [{low}-{high}]"
            )));
        }
        if high > MAX {
            return Err(D::Error::custom(format!(
                "Value provider too high: {MAX} [{low}-{high}]"
            )));
        }
        Ok(BoundedIntProvider(provider))
    }
}

impl IntProvider {
    pub fn uniform(min_inclusive: i32, max_inclusive: i32) -> Self {
        IntProvider::Dispatched(DispatchedIntProvider::Uniform {
            min_inclusive,
            max_inclusive,
        })
    }

    /// `getMinValue` / `getMaxValue`, which a few features read rather than
    /// sample.
    pub fn bounds(&self) -> (i32, i32) {
        use DispatchedIntProvider::*;
        match self {
            IntProvider::Constant(value) => (*value, *value),
            IntProvider::Dispatched(
                Uniform {
                    min_inclusive,
                    max_inclusive,
                }
                | BiasedToBottom {
                    min_inclusive,
                    max_inclusive,
                }
                | VeryBiasedToBottom {
                    min_inclusive,
                    max_inclusive,
                }
                | ClampedNormal {
                    min_inclusive,
                    max_inclusive,
                    ..
                },
            ) => (*min_inclusive, *max_inclusive),
            IntProvider::Dispatched(Clamped {
                source,
                min_inclusive,
                max_inclusive,
            }) => {
                let (low, high) = source.bounds();
                (low.max(*min_inclusive), high.min(*max_inclusive))
            }
            IntProvider::Dispatched(WeightedList { distribution }) => distribution
                .iter()
                .map(|entry| entry.data.bounds())
                .reduce(|(low, high), (next_low, next_high)| {
                    (low.min(next_low), high.max(next_high))
                })
                .expect("a weighted list is non-empty by its own deserializer"),
            IntProvider::Dispatched(Trapezoid { min, max, .. }) => (*min, *max),
        }
    }

    pub fn sample<R: Random>(&self, rng: &mut R) -> i32 {
        use DispatchedIntProvider::*;
        match self {
            Self::Constant(value) => *value,
            Self::Dispatched(Uniform {
                min_inclusive,
                max_inclusive,
            }) => rng.next_int_between_inclusive(*min_inclusive, *max_inclusive),
            Self::Dispatched(BiasedToBottom {
                min_inclusive,
                max_inclusive,
            }) => {
                let span = rng.next_i32_bound(max_inclusive - min_inclusive + 1) + 1;
                min_inclusive + rng.next_i32_bound(span)
            }
            Self::Dispatched(VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
            }) => {
                let span = rng.next_i32_bound(max_inclusive - min_inclusive + 1) + 1;
                let span = rng.next_i32_bound(span) + 1;
                min_inclusive + rng.next_i32_bound(span)
            }
            Self::Dispatched(Clamped {
                source,
                min_inclusive,
                max_inclusive,
            }) => source.sample(rng).clamp(*min_inclusive, *max_inclusive),
            Self::Dispatched(WeightedList { distribution }) => {
                pick_weighted(distribution, rng).sample(rng)
            }
            Self::Dispatched(ClampedNormal {
                mean,
                deviation,
                min_inclusive,
                max_inclusive,
            }) => {
                normal(rng, *mean, *deviation).clamp(*min_inclusive as f32, *max_inclusive as f32)
                    as i32
            }
            Self::Dispatched(Trapezoid { min, max, plateau }) => {
                if *plateau == 0 && *max == -*min {
                    rng.next_i32_bound(max + 1) - rng.next_i32_bound(max + 1)
                } else {
                    let range = max - min;
                    if *plateau == range {
                        rng.next_int_between_inclusive(*min, *max)
                    } else {
                        let plateau_start = (range - plateau) / 2;
                        let plateau_end = range - plateau_start;
                        min + rng.next_int_between_inclusive(0, plateau_end)
                            + rng.next_int_between_inclusive(0, plateau_start)
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FloatProvider {
    Constant(f32),
    Dispatched(DispatchedFloatProvider),
}

impl<'de> Deserialize<'de> for FloatProvider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ConstantOrDispatch::deserialize(deserializer)?.split() {
            Ok(value) => Ok(Self::Constant(value)),
            Err(dispatched) => {
                // `UniformFloat` alone refuses an empty span.
                if let DispatchedFloatProvider::Uniform {
                    min_inclusive,
                    max_exclusive,
                } = dispatched
                    && max_exclusive <= min_inclusive
                {
                    return Err(D::Error::custom(format!(
                        "Max must be larger than min, min: {min_inclusive}, max: {max_exclusive}"
                    )));
                }
                span_ordered(dispatched.span()).map_err(D::Error::custom)?;
                Ok(Self::Dispatched(dispatched))
            }
        }
    }
}

impl DispatchedFloatProvider {
    fn span(&self) -> Option<(f32, f32, Option<f32>)> {
        use DispatchedFloatProvider::*;
        match self {
            Uniform { .. } => None,
            ClampedNormal { min, max, .. } => Some((*min, *max, None)),
            Trapezoid { min, max, plateau } => Some((*min, *max, Some(*plateau))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum DispatchedFloatProvider {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    #[serde(rename = "minecraft:clamped_normal")]
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min: f32,
        max: f32,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid { min: f32, max: f32, plateau: f32 },
}

impl FloatProvider {
    pub fn sample<R: Random>(self, rng: &mut R) -> f32 {
        use DispatchedFloatProvider::*;
        match self {
            Self::Constant(value) => value,
            Self::Dispatched(Uniform {
                min_inclusive,
                max_exclusive,
            }) => rng.next_f32() * (max_exclusive - min_inclusive) + min_inclusive,
            Self::Dispatched(ClampedNormal {
                mean,
                deviation,
                min,
                max,
            }) => normal(rng, mean, deviation).clamp(min, max),
            Self::Dispatched(Trapezoid { min, max, plateau }) => {
                let range = max - min;
                let ramp = (range - plateau) / 2.0;
                min + rng.next_f32() * (range - ramp) + rng.next_f32() * ramp
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum HeightProvider {
    Constant(VerticalAnchor),
    Dispatched(DispatchedHeightProvider),
}

impl<'de> Deserialize<'de> for HeightProvider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ConstantOrDispatch::deserialize(deserializer)?.split() {
            Ok(value) => Ok(Self::Constant(value)),
            Err(dispatched) => Ok(Self::Dispatched(dispatched)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum DispatchedHeightProvider {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    #[serde(rename = "minecraft:very_biased_to_bottom")]
    VeryBiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        #[serde(default, skip_serializing_if = "is_default")]
        inner: Bounded<{ i32::MIN }, { i32::MAX }, 1>,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        #[serde(default, skip_serializing_if = "is_default")]
        plateau: i32,
    },
}

impl HeightProvider {
    pub fn sample<R: Random>(self, rng: &mut R, context: HeightContext) -> i32 {
        use DispatchedHeightProvider::*;
        match self {
            Self::Constant(anchor) => anchor.resolve_y(context),
            Self::Dispatched(Uniform {
                min_inclusive,
                max_inclusive,
            }) => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if min > max {
                    return min;
                }
                rng.next_int_between_inclusive(min, max)
            }
            Self::Dispatched(VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            }) => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                let inner = inner.0;
                if max - min - inner + 1 <= 0 {
                    return min;
                }
                let upper = next_int_guarded(rng, min + inner, max);
                let biased_upper = next_int_guarded(rng, min, upper - 1);
                next_int_guarded(rng, min, biased_upper - 1 + inner)
            }
            Self::Dispatched(Trapezoid {
                min_inclusive,
                max_inclusive,
                plateau,
            }) => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if min > max {
                    return min;
                }
                let range = max - min;
                if plateau >= range {
                    return rng.next_int_between_inclusive(min, max);
                }
                let plateau_start = (range - plateau) / 2;
                let plateau_end = range - plateau_start;
                min + rng.next_int_between_inclusive(0, plateau_end)
                    + rng.next_int_between_inclusive(0, plateau_start)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::Bounded;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    const OVERWORLD: HeightContext = HeightContext {
        min_y: -64,
        depth: 384,
        sea_level: 63,
    };

    fn round_trip<T>(json: &str) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let parsed: T = serde_json::from_str(json).expect("parses");
        let written = serde_json::to_string(&parsed).expect("writes");
        let again: T = serde_json::from_str(&written).expect("reparses");
        assert_eq!(parsed, again, "round trip changed the value: {written}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            serde_json::from_str::<serde_json::Value>(&written).unwrap(),
            "round trip changed the json"
        );
        parsed
    }

    /// Runs a sampler and an independently written replay of its reference draw
    /// sequence from the same seed. Equal values prove the arithmetic, equal RNG
    /// state proves the number and order of draws.
    #[track_caller]
    fn pin_draws<T: PartialEq + std::fmt::Debug>(
        sample: impl FnOnce(&mut XoroshiroRandom) -> T,
        replay: impl FnOnce(&mut XoroshiroRandom) -> T,
    ) -> T {
        let mut sampled = XoroshiroRandom::new(0x5eed);
        let mut reference = XoroshiroRandom::new(0x5eed);
        let value = sample(&mut sampled);
        assert_eq!(value, replay(&mut reference), "sampled value");
        assert_eq!(sampled, reference, "draw sequence");
        value
    }

    #[test]
    fn a_bare_number_is_a_constant() {
        assert_eq!(round_trip::<IntProvider>("7"), IntProvider::Constant(7));
        assert_eq!(
            round_trip::<FloatProvider>("-0.7"),
            FloatProvider::Constant(-0.7)
        );
    }

    /// The corpus never writes it, but `constant` is a member of all three provider
    /// registries, so a datapack may. The reference encodes a constant through
    /// `Either.left(value)`, so the object form is accepted and written back bare.
    #[test]
    fn the_explicit_constant_form_loads_and_normalises() {
        assert_eq!(
            serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:constant","value":7}"#)
                .expect("parses"),
            IntProvider::Constant(7)
        );
        assert_eq!(
            serde_json::from_str::<FloatProvider>(r#"{"type":"minecraft:constant","value":-0.7}"#)
                .expect("parses"),
            FloatProvider::Constant(-0.7)
        );
        assert_eq!(
            serde_json::from_str::<HeightProvider>(
                r#"{"type":"minecraft:constant","value":{"below_top":1}}"#
            )
            .expect("parses"),
            HeightProvider::Constant(VerticalAnchor::BelowTop(1))
        );

        assert_eq!(
            serde_json::to_string(&IntProvider::Constant(7)).expect("writes"),
            "7"
        );
        assert_eq!(
            serde_json::to_string(&FloatProvider::Constant(-0.7)).expect("writes"),
            "-0.7"
        );
        assert_eq!(
            serde_json::to_string(&HeightProvider::Constant(VerticalAnchor::BelowTop(1)))
                .expect("writes"),
            r#"{"below_top":1}"#
        );

        assert!(
            serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:constant","value":7,"x":1}"#)
                .is_err(),
            "an unknown sibling field is a load error"
        );
    }

    #[test]
    fn the_carver_registry_forms_round_trip() {
        assert_eq!(
            round_trip::<IntProvider>(
                r#"{"type":"minecraft:very_biased_to_bottom","min_inclusive":0,"max_inclusive":14}"#
            ),
            IntProvider::Dispatched(DispatchedIntProvider::VeryBiasedToBottom {
                min_inclusive: 0,
                max_inclusive: 14
            })
        );
        assert_eq!(
            round_trip::<FloatProvider>(
                r#"{"type":"minecraft:trapezoid","min":0.0,"max":3.0,"plateau":1.0}"#
            ),
            FloatProvider::Dispatched(DispatchedFloatProvider::Trapezoid {
                min: 0.0,
                max: 3.0,
                plateau: 1.0
            })
        );
        assert_eq!(
            round_trip::<FloatProvider>(
                r#"{"type":"minecraft:uniform","min_inclusive":0.7,"max_exclusive":1.4}"#
            ),
            FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
                min_inclusive: 0.7,
                max_exclusive: 1.4
            })
        );
        assert_eq!(
            round_trip::<HeightProvider>(
                r#"{"type":"minecraft:uniform","min_inclusive":{"above_bottom":8},"max_inclusive":{"absolute":180}}"#
            ),
            HeightProvider::Dispatched(DispatchedHeightProvider::Uniform {
                min_inclusive: VerticalAnchor::AboveBottom(8),
                max_inclusive: VerticalAnchor::Absolute(180)
            })
        );
        assert_eq!(
            round_trip::<HeightProvider>(r#"{"below_top":1}"#),
            HeightProvider::Constant(VerticalAnchor::BelowTop(1))
        );
        assert_eq!(
            round_trip::<HeightProvider>(r#"{"relative_to_sea_level":0}"#),
            HeightProvider::Constant(VerticalAnchor::RelativeToSeaLevel(0))
        );
    }

    /// Every shape the 26.3 feature and placed_feature corpus carries, verbatim.
    #[test]
    fn the_feature_registry_forms_round_trip() {
        round_trip::<IntProvider>(r#"{"type":"minecraft:trapezoid","max":7,"min":-7,"plateau":0}"#);
        round_trip::<IntProvider>(
            r#"{"type":"minecraft:biased_to_bottom","max_inclusive":2,"min_inclusive":1}"#,
        );
        round_trip::<IntProvider>(
            r#"{"type":"minecraft:clamped","max_inclusive":1,"min_inclusive":0,"source":{"type":"minecraft:uniform","max_inclusive":1,"min_inclusive":-3}}"#,
        );
        round_trip::<IntProvider>(
            r#"{"type":"minecraft:clamped_normal","deviation":3.0,"max_inclusive":10,"mean":0.0,"min_inclusive":-10}"#,
        );
        round_trip::<IntProvider>(
            r#"{"type":"minecraft:weighted_list","distribution":[{"data":0,"weight":3},{"data":{"type":"minecraft:uniform","max_inclusive":4,"min_inclusive":1},"weight":1}]}"#,
        );
        round_trip::<FloatProvider>(
            r#"{"type":"minecraft:clamped_normal","deviation":0.3,"max":0.9,"mean":0.1,"min":0.1}"#,
        );
        round_trip::<HeightProvider>(
            r#"{"type":"minecraft:trapezoid","max_inclusive":{"absolute":192},"min_inclusive":{"absolute":0}}"#,
        );
        round_trip::<HeightProvider>(
            r#"{"type":"minecraft:very_biased_to_bottom","inner":8,"max_inclusive":{"below_top":8},"min_inclusive":{"above_bottom":0}}"#,
        );
    }

    /// `plateau` and `inner` are `optionalFieldOf` in the reference, so their
    /// defaults must not be written back out.
    #[test]
    fn the_height_defaults_are_omitted_on_the_way_out() {
        let trapezoid = HeightProvider::Dispatched(DispatchedHeightProvider::Trapezoid {
            min_inclusive: VerticalAnchor::Absolute(0),
            max_inclusive: VerticalAnchor::Absolute(16),
            plateau: 0,
        });
        assert!(
            !serde_json::to_string(&trapezoid)
                .unwrap()
                .contains("plateau")
        );
        let biased = HeightProvider::Dispatched(DispatchedHeightProvider::VeryBiasedToBottom {
            min_inclusive: VerticalAnchor::Absolute(0),
            max_inclusive: VerticalAnchor::Absolute(16),
            inner: Bounded(1),
        });
        assert!(!serde_json::to_string(&biased).unwrap().contains("inner"));
    }

    #[test]
    fn an_unnamed_form_is_a_load_error() {
        assert!(
            serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:not_a_provider"}"#).is_err()
        );
        assert!(serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:clamped"}"#).is_err());
        assert!(
            serde_json::from_str::<HeightProvider>(
                r#"{"type":"minecraft:weighted_list","distribution":[]}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<IntProvider>(
                r#"{"type":"minecraft:weighted_list","distribution":[]}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<IntProvider>(
                r#"{"type":"minecraft:weighted_list","distribution":[{"data":1,"weight":0}]}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<FloatProvider>(
                r#"{"type":"minecraft:uniform","min_inclusive":0.0,"max_exclusive":1.0,"extra":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn an_anchor_resolves_against_the_dimension_extent() {
        assert_eq!(VerticalAnchor::Absolute(180).resolve_y(OVERWORLD), 180);
        assert_eq!(VerticalAnchor::AboveBottom(8).resolve_y(OVERWORLD), -56);
        assert_eq!(VerticalAnchor::BelowTop(1).resolve_y(OVERWORLD), 318);
        assert_eq!(
            VerticalAnchor::RelativeToSeaLevel(3).resolve_y(OVERWORLD),
            66
        );
    }

    /// Every provider draws exactly the values Java's own `sample` does, in the
    /// same order, so a shared seed has to produce the same numbers.
    #[test]
    fn sampling_matches_the_reference_draw_order() {
        let mut rng = LegacyRandom::new(42);
        let mut reference = LegacyRandom::new(42);

        let uniform_int = IntProvider::Dispatched(DispatchedIntProvider::Uniform {
            min_inclusive: 3,
            max_inclusive: 9,
        });
        assert_eq!(
            uniform_int.sample(&mut rng),
            reference.next_i32_bound(9 - 3 + 1) + 3
        );

        let very_biased = IntProvider::Dispatched(DispatchedIntProvider::VeryBiasedToBottom {
            min_inclusive: 0,
            max_inclusive: 14,
        });
        let expected = {
            let a = reference.next_i32_bound(15) + 1;
            let b = reference.next_i32_bound(a) + 1;
            reference.next_i32_bound(b)
        };
        assert_eq!(very_biased.sample(&mut rng), expected);

        let trapezoid = FloatProvider::Dispatched(DispatchedFloatProvider::Trapezoid {
            min: 0.0,
            max: 3.0,
            plateau: 1.0,
        });
        let expected = {
            let range = 3.0f32;
            let ramp = (range - 1.0) / 2.0;
            reference.next_f32() * (range - ramp) + reference.next_f32() * ramp
        };
        assert_eq!(trapezoid.sample(&mut rng), expected);
    }

    #[test]
    fn biased_to_bottom_spends_two_draws() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::BiasedToBottom {
            min_inclusive: 1,
            max_inclusive: 2,
        });
        pin_draws(
            |rng| provider.sample(rng),
            |rng| {
                let span = rng.next_i32_bound(2) + 1;
                1 + rng.next_i32_bound(span)
            },
        );
    }

    #[test]
    fn clamped_spends_only_its_sources_draws() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::Clamped {
            source: Box::new(IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                min_inclusive: -3,
                max_inclusive: 1,
            })),
            min_inclusive: 0,
            max_inclusive: 1,
        });
        pin_draws(
            |rng| provider.sample(rng),
            |rng| (rng.next_i32_bound(5) - 3).clamp(0, 1),
        );
    }

    #[test]
    fn a_weighted_list_draws_the_selection_then_the_entry() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::WeightedList {
            distribution: vec![
                Weighted {
                    data: IntProvider::Constant(5),
                    weight: Bounded(3),
                },
                Weighted {
                    data: IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                        min_inclusive: 10,
                        max_inclusive: 12,
                    }),
                    weight: Bounded(1),
                },
            ],
        });
        pin_draws(
            |rng| provider.sample(rng),
            |rng| {
                if rng.next_i32_bound(4) < 3 {
                    5
                } else {
                    rng.next_i32_bound(3) + 10
                }
            },
        );
    }

    /// `IntProviders.codec(0, 16)` judges a provider by its whole range, so a
    /// uniform that reaches past the bound is refused even though every value
    /// it could draw might be in range on a given day.
    #[test]
    fn a_bounded_provider_is_judged_by_its_range() {
        type Size = BoundedIntProvider<0, 16>;
        assert_eq!(
            serde_json::from_str::<Size>("7").unwrap().0,
            IntProvider::Constant(7)
        );
        let uniform = r#"{"type":"minecraft:uniform","min_inclusive":2,"max_inclusive":16}"#;
        assert_eq!(
            serde_json::from_str::<Size>(uniform).unwrap().0,
            IntProvider::uniform(2, 16)
        );
        let too_high = r#"{"type":"minecraft:uniform","min_inclusive":2,"max_inclusive":17}"#;
        let error = serde_json::from_str::<Size>(too_high)
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("Value provider too high: 16 [2-17]"),
            "{error}"
        );
        let error = serde_json::from_str::<Size>("-1").unwrap_err().to_string();
        assert!(
            error.starts_with("Value provider too low: 0 [-1--1]"),
            "{error}"
        );
    }

    #[test]
    fn clamped_normal_spends_one_gaussian() {
        let int = IntProvider::Dispatched(DispatchedIntProvider::ClampedNormal {
            mean: 0.0,
            deviation: 3.0,
            min_inclusive: -10,
            max_inclusive: 10,
        });
        pin_draws(
            |rng| int.sample(rng),
            |rng| (rng.next_gaussian() as f32 * 3.0).clamp(-10.0, 10.0) as i32,
        );

        let float = FloatProvider::Dispatched(DispatchedFloatProvider::ClampedNormal {
            mean: 0.1,
            deviation: 0.3,
            min: 0.1,
            max: 0.9,
        });
        pin_draws(
            |rng| float.sample(rng),
            |rng| (0.1 + rng.next_gaussian() as f32 * 0.3).clamp(0.1, 0.9),
        );
    }

    /// The symmetric zero-plateau form the `offset` modifier uses is a difference
    /// of two draws, not the two-ramp sum.
    #[test]
    fn the_symmetric_int_trapezoid_is_a_difference_of_two_draws() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::Trapezoid {
            min: -7,
            max: 7,
            plateau: 0,
        });
        let value = pin_draws(
            |rng| provider.sample(rng),
            |rng| rng.next_i32_bound(8) - rng.next_i32_bound(8),
        );
        assert!((-7..=7).contains(&value));
    }

    #[test]
    fn an_asymmetric_int_trapezoid_sums_two_ramps() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::Trapezoid {
            min: 2,
            max: 11,
            plateau: 3,
        });
        pin_draws(
            |rng| provider.sample(rng),
            |rng| 2 + rng.next_i32_bound(7) + rng.next_i32_bound(4),
        );
    }

    #[test]
    fn a_full_plateau_int_trapezoid_is_one_uniform_draw() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::Trapezoid {
            min: 2,
            max: 11,
            plateau: 9,
        });
        pin_draws(|rng| provider.sample(rng), |rng| rng.next_i32_bound(10) + 2);
    }

    #[test]
    fn the_height_trapezoid_sums_two_ramps() {
        let provider = HeightProvider::Dispatched(DispatchedHeightProvider::Trapezoid {
            min_inclusive: VerticalAnchor::Absolute(0),
            max_inclusive: VerticalAnchor::Absolute(192),
            plateau: 0,
        });
        pin_draws(
            |rng| provider.sample(rng, OVERWORLD),
            |rng| rng.next_i32_bound(97) + rng.next_i32_bound(97),
        );
    }

    /// `VeryBiasedToBottomHeight` is not the int provider of the same name: three
    /// guarded draws over shrinking ranges, offset by `inner`.
    #[test]
    fn the_height_very_biased_to_bottom_spends_three_guarded_draws() {
        let provider = HeightProvider::Dispatched(DispatchedHeightProvider::VeryBiasedToBottom {
            min_inclusive: VerticalAnchor::AboveBottom(0),
            max_inclusive: VerticalAnchor::BelowTop(8),
            inner: Bounded(8),
        });
        let value = pin_draws(
            |rng| provider.sample(rng, OVERWORLD),
            |rng| {
                let (min, max) = (-64, 311);
                let upper = rng.next_i32_bound(max - (min + 8) + 1) + min + 8;
                let biased_upper = rng.next_i32_bound(upper - 1 - min + 1) + min;
                let top = biased_upper - 1 + 8;
                if min >= top {
                    min
                } else {
                    rng.next_i32_bound(top - min + 1) + min
                }
            },
        );
        assert!((-64..=311).contains(&value), "{value} out of range");
    }

    #[test]
    fn an_empty_height_range_returns_the_floor_without_drawing() {
        let provider = HeightProvider::Dispatched(DispatchedHeightProvider::VeryBiasedToBottom {
            min_inclusive: VerticalAnchor::Absolute(10),
            max_inclusive: VerticalAnchor::Absolute(12),
            inner: Bounded(8),
        });
        pin_draws(|rng| provider.sample(rng, OVERWORLD), |_| 10);
    }

    #[test]
    fn a_very_biased_sample_stays_in_range_and_leans_low() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::VeryBiasedToBottom {
            min_inclusive: 0,
            max_inclusive: 14,
        });
        let mut rng = LegacyRandom::new(7);
        let mut zeroes = 0;
        for _ in 0..2000 {
            let value = provider.sample(&mut rng);
            assert!((0..=14).contains(&value), "{value} out of range");
            if value == 0 {
                zeroes += 1;
            }
        }
        assert!(zeroes > 600, "only {zeroes} zeroes of 2000");
    }

    #[test]
    fn bounds_walk_a_clamped_source() {
        let provider = IntProvider::Dispatched(DispatchedIntProvider::Clamped {
            source: Box::new(IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                min_inclusive: 3,
                max_inclusive: 19,
            })),
            min_inclusive: 3,
            max_inclusive: 16,
        });
        assert_eq!(provider.bounds(), (3, 16));
    }
}
