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

impl<L, R> Either<L, R> {
    pub fn left(&self) -> Option<&L> {
        match self {
            Either::Left(l) => Some(l),
            Either::Right(_) => None,
        }
    }

    pub fn right(&self) -> Option<&R> {
        match self {
            Either::Left(_) => None,
            Either::Right(r) => Some(r),
        }
    }

    pub fn map<T>(self, left: impl FnOnce(L) -> T, right: impl FnOnce(R) -> T) -> T {
        match self {
            Either::Left(l) => left(l),
            Either::Right(r) => right(r),
        }
    }
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
#[serde(from = "Either<I, Either<[I; 2], NamedRange<I>>>")]
#[serde(into = "Either<I, Either<[I; 2], NamedRange<I>>>")]
pub struct ValueRange<I>
where
    I: Clone + PartialEq,
{
    pub min: I,
    pub max: I,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct NamedRange<I> {
    min: I,
    max: I,
}

impl<I: Clone + PartialEq> From<Either<I, Either<[I; 2], NamedRange<I>>>> for ValueRange<I> {
    fn from(value: Either<I, Either<[I; 2], NamedRange<I>>>) -> Self {
        match value {
            Either::Left(value) => ValueRange {
                min: value.clone(),
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

impl<I: Clone + PartialEq> From<ValueRange<I>> for Either<I, Either<[I; 2], NamedRange<I>>> {
    fn from(value: ValueRange<I>) -> Self {
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
        let bare: ValueRange<f64> = serde_json::from_str("0.5").unwrap();
        assert_eq!((bare.min, bare.max), (0.5, 0.5));
        let pair: ValueRange<f64> = serde_json::from_str("[-1.0, 1.0]").unwrap();
        assert_eq!((pair.min, pair.max), (-1.0, 1.0));
        let named: ValueRange<f64> = serde_json::from_str(r#"{"min":-1.0,"max":1.0}"#).unwrap();
        assert_eq!((named.min, named.max), (-1.0, 1.0));
        assert_eq!(serde_json::to_string(&pair).unwrap(), "[-1.0,1.0]");
    }
}
