use crate::constant_or_dispatch::ConstantOrDispatch;
use crate::sample::next_int_guarded;
use crate::{HeightContext, VerticalAnchor};
use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_registries::dispatch;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum HeightProvider {
    Constant(VerticalAnchor),
    Dispatched(DispatchedHeightProvider),
}

impl<'de> Deserialize<'de> for HeightProvider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ConstantOrDispatch::deserialize(deserializer, true)? {
            ConstantOrDispatch::Bare(value)
            | ConstantOrDispatch::Dispatched(DispatchedHeightProvider::Constant { value }) => {
                Ok(Self::Constant(value))
            }
            ConstantOrDispatch::Dispatched(dispatched) => Ok(Self::Dispatched(dispatched)),
        }
    }
}

#[dispatch(registry = crate::keys::HeightProviderType, unsupported(BiasedToBottom, WeightedList))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub enum DispatchedHeightProvider {
    Constant {
        value: VerticalAnchor,
    },
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    VeryBiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        #[serde(default, skip_serializing_if = "is_default")]
        inner: Bounded<{ i32::MIN }, { i32::MAX }, 1>,
    },
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
            Self::Constant(anchor) | Self::Dispatched(Constant { value: anchor }) => {
                anchor.resolve_y(context)
            }
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
                if max - min - inner < 0 {
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
