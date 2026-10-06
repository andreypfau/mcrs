use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_core::value_provider::{IntProvider, Weighted};
use mcrs_minecraft_core::{Direction, ResourceKey, ResourceLocation};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::de::Error as _;
use serde::de::{MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::block_state::BlockState;
use crate::predicate::BlockPredicate;

mcrs_minecraft_worldgen_noise::bounded_float! {
    /// `Codec.floatRange(0.0F, 1.0F)`.
    UnitFloat as f32 in [0.0, 1.0];
    /// `Codec.floatRange(-1.0F, 1.0F)`.
    SignedUnitFloat as f32 in [-1.0, 1.0];
}

/// `ExtraCodecs.POSITIVE_FLOAT`: the low bound is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64")]
pub struct PositiveFloat(pub f64);

impl TryFrom<f64> for PositiveFloat {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, String> {
        let narrowed = value as f32;
        if narrowed.is_nan() || narrowed <= 0.0 {
            return Err(format!("Value must be positive: {value}"));
        }
        Ok(PositiveFloat(value))
    }
}

pub fn non_empty<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let values = Vec::<T>::deserialize(deserializer)?;
    if values.is_empty() {
        return Err(D::Error::custom("List must have contents"));
    }
    Ok(values)
}

/// `RegistryCodecs.holderSet(Registries.BLOCK)`.
pub type BlockSet = HolderSet<keys::Block>;

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
    State(FullBlockState),
    Typed(TypedBlockStateProvider),
}

/// `BlockState.FULL_CODEC`. A singleton block writes no `properties`, which
/// must not come back as an empty map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullBlockState {
    pub id: ResourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, String>>,
}

impl FullBlockState {
    pub fn state(&self) -> BlockState {
        BlockState {
            name: self.id.clone(),
            properties: self.properties.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TypedBlockStateProvider {
    #[serde(rename = "minecraft:simple")]
    Simple { state: BlockState },
    #[serde(rename = "minecraft:weighted")]
    Weighted {
        #[serde(deserialize_with = "non_empty")]
        entries: Vec<Weighted<BlockState>>,
    },
    #[serde(rename = "minecraft:rule_based")]
    RuleBased {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<BlockStateProvider>,
        rules: Vec<StateRule>,
    },
    #[serde(rename = "minecraft:randomized_int")]
    RandomizedInt {
        source: BlockStateProvider,
        property: String,
        values: IntProvider,
    },
    #[serde(rename = "minecraft:rotated")]
    Rotated {
        state: BlockStateProvider,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<Direction>,
    },
    #[serde(rename = "minecraft:random_block")]
    RandomBlock { blocks: BlockSet },
    #[serde(rename = "minecraft:copy_properties")]
    CopyProperties { source: BlockStateProvider },
    #[serde(rename = "minecraft:noise")]
    Noise {
        seed: i64,
        noise: NoiseParam,
        scale: PositiveFloat,
        #[serde(deserialize_with = "non_empty")]
        states: Vec<BlockState>,
    },
    #[serde(rename = "minecraft:noise_threshold")]
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
    #[serde(rename = "minecraft:dual_noise")]
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

const BLOCK_STATE_PROVIDER_TYPE_ROWS: &[&str] = &[
    "minecraft:simple",
    "minecraft:weighted",
    "minecraft:rule_based",
    "minecraft:randomized_int",
    "minecraft:rotated",
    "minecraft:random_block",
    "minecraft:copy_properties",
    "minecraft:noise",
    "minecraft:noise_threshold",
    "minecraft:dual_noise",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    BLOCK_STATE_PROVIDER_TYPE_ROWS,
    &[],
    keys::block_state_provider_type::ENTRIES
));

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRule {
    pub if_true: BlockPredicate,
    pub then: BlockStateProvider,
}

fn resource_location<E: serde::de::Error>(id: &str) -> Result<ResourceLocation, E> {
    ResourceLocation::from_str(id).map_err(|_| E::custom(format!("Not a valid id: {id}")))
}

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn block_state_provider_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<TypedBlockStateProvider>(
            BLOCK_STATE_PROVIDER_TYPE_ROWS,
            &[],
            keys::block_state_provider_type::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
