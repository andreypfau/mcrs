use crate::proto::HashableF64;
use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `Codec.either(A, B)`: a value written either as `A` or as `B`, round-tripping
/// back into the shape it came from.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

/// A block state as the noise settings write it: a bare block id, or an id with
/// stated property values.
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub struct BlockState {
    pub name: ResourceLocation,
    /// `None` where the file named the block alone, which is not the same as an
    /// empty property map: the two must serialize back to what they came from.
    pub properties: Option<BTreeMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatedBlockState {
    id: ResourceLocation,
    #[serde(default)]
    properties: BTreeMap<String, String>,
}

impl<'de> Deserialize<'de> for BlockState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Either::<ResourceLocation, StatedBlockState>::deserialize(deserializer)? {
            Either::Left(name) => Ok(BlockState {
                name,
                properties: None,
            }),
            Either::Right(state) => Ok(BlockState {
                name: state.id,
                properties: Some(state.properties),
            }),
        }
    }
}

impl Serialize for BlockState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.properties {
            None => self.name.serialize(serializer),
            Some(properties) => StatedBlockState {
                id: self.name.clone(),
                properties: properties.clone(),
            }
            .serialize(serializer),
        }
    }
}

/// A closed range written as a bare value, a two-element array, or an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "RangeForm")]
#[serde(into = "RangeForm")]
pub struct ValueRange {
    pub min: HashableF64,
    pub max: HashableF64,
}

type RangeForm = Either<HashableF64, Either<[HashableF64; 2], NamedRange>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct NamedRange {
    min: HashableF64,
    max: HashableF64,
}

impl From<RangeForm> for ValueRange {
    fn from(value: RangeForm) -> Self {
        match value {
            Either::Left(value) => ValueRange {
                min: value,
                max: value,
            },
            Either::Right(Either::Left([min, max])) => ValueRange { min, max },
            Either::Right(Either::Right(range)) => ValueRange {
                min: range.min,
                max: range.max,
            },
        }
    }
}

impl From<ValueRange> for RangeForm {
    fn from(value: ValueRange) -> Self {
        if value.min == value.max {
            Either::Left(value.min)
        } else {
            Either::Right(Either::Left([value.min, value.max]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_block_id_round_trips_without_a_property_map() {
        let state: BlockState = serde_json::from_str(r#""minecraft:stone""#).unwrap();
        assert_eq!(state.name.as_str(), "minecraft:stone");
        assert!(state.properties.is_none());
        assert_eq!(
            serde_json::to_string(&state).unwrap(),
            r#""minecraft:stone""#
        );
    }

    #[test]
    fn a_stated_block_keeps_its_properties() {
        let state: BlockState =
            serde_json::from_str(r#"{"id":"minecraft:water","properties":{"level":"0"}}"#).unwrap();
        assert_eq!(state.properties.as_ref().unwrap()["level"], "0");
        assert_eq!(
            serde_json::to_string(&state).unwrap(),
            r#"{"id":"minecraft:water","properties":{"level":"0"}}"#
        );
    }

    #[test]
    fn a_range_reads_all_three_shapes() {
        let bare: ValueRange = serde_json::from_str("0.5").unwrap();
        assert_eq!((bare.min.0, bare.max.0), (0.5, 0.5));
        let pair: ValueRange = serde_json::from_str("[-1.0, 1.0]").unwrap();
        assert_eq!((pair.min.0, pair.max.0), (-1.0, 1.0));
        let named: ValueRange = serde_json::from_str(r#"{"min":-1.0,"max":1.0}"#).unwrap();
        assert_eq!((named.min.0, named.max.0), (-1.0, 1.0));
        assert_eq!(serde_json::to_string(&pair).unwrap(), "[-1.0,1.0]");
    }
}
