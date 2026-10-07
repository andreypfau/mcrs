use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use mcrs_minecraft_core::codec::{PositiveFloat, non_empty};
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_core::{Direction, ResourceKey, ResourceLocation};
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_value_provider::{IntProvider, Weighted};
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::de::Error as _;
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::block_state::{BlockState, serialize_stated};
use crate::predicate::BlockPredicate;
use mcrs_minecraft_block::keys::Block;

mcrs_minecraft_worldgen_noise::bounded_float! {
    /// `Codec.floatRange(0.0F, 1.0F)`.
    UnitFloat as f32 in [0.0, 1.0];
    /// `Codec.floatRange(-1.0F, 1.0F)`.
    SignedUnitFloat as f32 in [-1.0, 1.0];
}

/// `RegistryCodecs.holderSet(Registries.BLOCK)`.
pub type BlockSet = HolderSet<Block>;

/// `ExtraCodecs.intervalCodec`: one point, a two-element array, or the named
/// pair. Vanilla re-encodes all three as the shortest form that fits, so the
/// three are kept apart to round-trip instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntRange {
    Point(i32),
    Pair([i32; 2]),
    Named {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

impl IntRange {
    pub fn min_inclusive(self) -> i32 {
        match self {
            IntRange::Point(value) => value,
            IntRange::Pair([min, _]) => min,
            IntRange::Named { min_inclusive, .. } => min_inclusive,
        }
    }

    pub fn max_inclusive(self) -> i32 {
        match self {
            IntRange::Point(value) => value,
            IntRange::Pair([_, max]) => max,
            IntRange::Named { max_inclusive, .. } => max_inclusive,
        }
    }
}

fn variety_range<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IntRange, D::Error> {
    let range = IntRange::deserialize(deserializer)?;
    if range.min_inclusive() < 1 {
        return Err(D::Error::custom(format!(
            "Range limit too low, expected at least 1 [{}-{}]",
            range.min_inclusive(),
            range.max_inclusive()
        )));
    }
    if range.max_inclusive() > 64 {
        return Err(D::Error::custom(format!(
            "Range limit too high, expected at most 64 [{}-{}]",
            range.min_inclusive(),
            range.max_inclusive()
        )));
    }
    Ok(range)
}

/// `RegistryCodecs.holder(registry, direct, allowInline = true)`: an id naming a
/// registry entry, or the entry itself written out in place.
#[derive(Debug, Clone, PartialEq)]
pub enum Holder<T> {
    Reference(ResourceLocation),
    Inline(Box<T>),
}

impl<V: RegistryValue> From<ResourceKey<V::Registry, &'static str>> for Holder<V> {
    fn from(key: ResourceKey<V::Registry, &'static str>) -> Self {
        Holder::Reference((*key.location()).into())
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Holder<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct HolderVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for HolderVisitor<T> {
            type Value = Holder<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a registry id or an inline entry")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<Self::Value, E> {
                Ok(Holder::Reference(resource_location(id)?))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(value::MapAccessDeserializer::new(map))
                    .map(|entry| Holder::Inline(Box::new(entry)))
            }
        }

        deserializer.deserialize_any(HolderVisitor(PhantomData))
    }
}

impl<T: Serialize> Serialize for Holder<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Holder::Reference(id) => id.serialize(serializer),
            Holder::Inline(entry) => entry.serialize(serializer),
        }
    }
}

pub type BlockStateProvider = Holder<DirectBlockStateProvider>;

/// `Codec.xor(BlockState.FULL_CODEC, TYPED_CODEC)`: a state object or a typed
/// provider, never a bare id, which the holder around it reads as a reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DirectBlockStateProvider {
    State(#[serde(serialize_with = "serialize_stated")] BlockState),
    Typed(TypedBlockStateProvider),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum TypedBlockStateProvider {
    Simple {
        state: BlockState,
    },
    Weighted {
        #[serde(deserialize_with = "non_empty")]
        entries: Vec<Weighted<BlockState>>,
    },
    RuleBased {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<BlockStateProvider>,
        rules: Vec<StateRule>,
    },
    RandomizedInt {
        source: BlockStateProvider,
        property: String,
        values: IntProvider,
    },
    Rotated {
        state: BlockStateProvider,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<Direction>,
    },
    RandomBlock {
        blocks: BlockSet,
    },
    CopyProperties {
        source: BlockStateProvider,
    },
    Noise {
        seed: i64,
        noise: NoiseParam,
        scale: PositiveFloat,
        #[serde(deserialize_with = "non_empty")]
        states: Vec<BlockState>,
    },
    NoiseThreshold {
        seed: i64,
        noise: NoiseParam,
        scale: PositiveFloat,
        threshold: SignedUnitFloat,
        high_chance: UnitFloat,
        default_state: BlockState,
        #[serde(deserialize_with = "non_empty")]
        low_states: Vec<BlockState>,
        #[serde(deserialize_with = "non_empty")]
        high_states: Vec<BlockState>,
    },
    DualNoise {
        #[serde(deserialize_with = "variety_range")]
        variety: IntRange,
        slow_noise: NoiseParam,
        slow_scale: PositiveFloat,
        seed: i64,
        noise: NoiseParam,
        scale: PositiveFloat,
        #[serde(deserialize_with = "non_empty")]
        states: Vec<BlockState>,
    },
}

mcrs_minecraft_registry::dispatch! {
    TypedBlockStateProvider, key = "type", registry = crate::keys::BlockStateProviderType,
    {
        CopyProperties => CopyProperties,
        DualNoise => DualNoise,
        Noise => Noise,
        NoiseThreshold => NoiseThreshold,
        RandomBlock => RandomBlock,
        RandomizedInt => RandomizedInt,
        Rotated => Rotated,
        RuleBased => RuleBased,
        Simple => Simple,
        Weighted => Weighted,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRule {
    pub if_true: BlockPredicate,
    pub then: BlockStateProvider,
}

fn resource_location<E: serde::de::Error>(id: &str) -> Result<ResourceLocation, E> {
    ResourceLocation::from_str(id).map_err(|_| E::custom(format!("Not a valid id: {id}")))
}
