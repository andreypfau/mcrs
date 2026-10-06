use mcrs_minecraft_worldgen_noise::proto::HashableF64;
use serde::{Deserialize, Serialize};

/// `Codec.either(A, B)`: a value written either as `A` or as `B`, round-tripping
/// back into the shape it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

/// A closed range written as a bare value, a two-element array, or an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "RangeForm")]
#[serde(into = "RangeForm")]
pub struct ValueRange {
    pub min: HashableF64,
    pub max: HashableF64,
}

impl ValueRange {
    pub fn new(min: f64, max: f64) -> Self {
        ValueRange {
            min: HashableF64(min),
            max: HashableF64(max),
        }
    }
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
