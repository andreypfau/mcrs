use crate::constant_or_dispatch::ConstantOrDispatch;
use crate::int_provider::span_ordered;
use crate::sample::normal;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_registries::dispatch;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FloatProvider {
    Constant(f32),
    Dispatched(DispatchedFloatProvider),
}

impl<'de> Deserialize<'de> for FloatProvider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ConstantOrDispatch::deserialize(deserializer, false)? {
            ConstantOrDispatch::Bare(value)
            | ConstantOrDispatch::Dispatched(DispatchedFloatProvider::Constant { value }) => {
                Ok(Self::Constant(value))
            }
            ConstantOrDispatch::Dispatched(dispatched) => {
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
            Constant { .. } | Uniform { .. } => None,
            ClampedNormal { min, max, .. } => Some((*min, *max, None)),
            Trapezoid { min, max, plateau } => Some((*min, *max, Some(*plateau))),
        }
    }
}

#[dispatch(registry = crate::keys::FloatProviderType)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum DispatchedFloatProvider {
    Constant {
        value: f32,
    },
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min: f32,
        max: f32,
    },
    Trapezoid {
        min: f32,
        max: f32,
        plateau: f32,
    },
}

impl FloatProvider {
    pub fn sample<R: Random>(self, rng: &mut R) -> f32 {
        use DispatchedFloatProvider::*;
        match self {
            Self::Constant(value) | Self::Dispatched(Constant { value }) => value,
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
