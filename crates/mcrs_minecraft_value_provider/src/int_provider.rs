use crate::WeightedDistribution;
use crate::constant_or_dispatch::ConstantOrDispatch;
use crate::sample::normal;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_registries::dispatch;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum IntProvider {
    Constant(i32),
    Dispatched(DispatchedIntProvider),
}

impl<'de> Deserialize<'de> for IntProvider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ConstantOrDispatch::<i32, DispatchedIntProvider>::deserialize(deserializer, false)? {
            ConstantOrDispatch::Bare(value)
            | ConstantOrDispatch::Dispatched(DispatchedIntProvider::Constant { value }) => {
                Ok(Self::Constant(value))
            }
            ConstantOrDispatch::Dispatched(dispatched) => {
                span_ordered(dispatched.span()).map_err(D::Error::custom)?;
                Ok(Self::Dispatched(dispatched))
            }
        }
    }
}

/// The range checks the reference's codecs make: a span whose max falls below
/// its min, or a plateau wider than the span, is a load error.
pub(crate) fn span_ordered<T: PartialOrd + std::ops::Sub<Output = T> + Copy + std::fmt::Display>(
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
            | VeryBiasedToBottom {
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
            Constant { .. } | WeightedList { .. } => None,
        }
    }
}

#[dispatch(registry = crate::keys::IntProviderType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum DispatchedIntProvider {
    Constant {
        value: i32,
    },
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    BiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    VeryBiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    Clamped {
        source: Box<IntProvider>,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    WeightedList {
        distribution: WeightedDistribution<IntProvider>,
    },
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    Trapezoid {
        min: i32,
        max: i32,
        plateau: i32,
    },
}

impl IntProvider {
    pub fn uniform(min_inclusive: i32, max_inclusive: i32) -> Self {
        IntProvider::Dispatched(DispatchedIntProvider::Uniform {
            min_inclusive,
            max_inclusive,
        })
    }

    /// A constant where the bounds meet, a uniform range otherwise.
    pub fn between(min_inclusive: i32, max_inclusive: i32) -> Self {
        if min_inclusive == max_inclusive {
            IntProvider::Constant(min_inclusive)
        } else {
            IntProvider::uniform(min_inclusive, max_inclusive)
        }
    }

    pub fn bounds(&self) -> (i32, i32) {
        use DispatchedIntProvider::*;
        match self {
            IntProvider::Constant(value) | IntProvider::Dispatched(Constant { value }) => {
                (*value, *value)
            }
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
                .rest
                .iter()
                .map(|entry| entry.data.bounds())
                .fold(
                    distribution.first.data.bounds(),
                    |(low, high), (next_low, next_high)| (low.min(next_low), high.max(next_high)),
                ),
            IntProvider::Dispatched(Trapezoid { min, max, .. }) => (*min, *max),
        }
    }

    pub fn sample<R: Random>(&self, rng: &mut R) -> i32 {
        use DispatchedIntProvider::*;
        match self {
            Self::Constant(value) | Self::Dispatched(Constant { value }) => *value,
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
            Self::Dispatched(WeightedList { distribution }) => distribution.pick(rng).sample(rng),
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
