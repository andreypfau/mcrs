use std::cmp::Ordering;
use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::codec::{Number, int_value};
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A bare number when both bounds agree, else `{min, max}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MinMaxBounds<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T> MinMaxBounds<T> {
    pub const ANY: Self = MinMaxBounds {
        min: None,
        max: None,
    };

    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    pub fn matches(&self, value: T) -> bool
    where
        T: PartialOrd + Copy,
    {
        !self.min.is_some_and(|min| min > value) && !self.max.is_some_and(|max| max < value)
    }
}

/// A bound read from any number, ordered and printed as its boxed Java type
/// is.
pub trait Bound: Copy + Serialize {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error>;
    fn compare(self, other: Self) -> Ordering;
    fn java_string(self) -> String;
}

impl Bound for i32 {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        int_value(d)
    }

    fn compare(self, other: Self) -> Ordering {
        self.cmp(&other)
    }

    fn java_string(self) -> String {
        self.to_string()
    }
}

/// `-0.0` orders below `0.0` and is not equal to it, unlike the primitive
/// operators.
impl Bound for f64 {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        f64::deserialize(d)
    }

    fn compare(self, other: Self) -> Ordering {
        self.total_cmp(&other)
    }

    fn java_string(self) -> String {
        mcrs_minecraft_snbt::java_double(self)
    }
}

impl<T: Bound> Serialize for MinMaxBounds<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if let (Some(min), Some(max)) = (self.min, self.max)
            && min.compare(max) == Ordering::Equal
        {
            return min.serialize(s);
        }
        let mut map = s.serialize_map(None)?;
        if let Some(min) = &self.min {
            map.serialize_entry("min", min)?;
        }
        if let Some(max) = &self.max {
            map.serialize_entry("max", max)?;
        }
        map.end()
    }
}

fn optional_bound<'de, T: Bound, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    struct OptionalBound<T>(PhantomData<T>);

    impl<'de, T: Bound> Visitor<'de> for OptionalBound<T> {
        type Value = Option<T>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a number")
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            T::read(d).map(Some)
        }
    }

    d.deserialize_option(OptionalBound(PhantomData))
}

impl<'de, T: Bound> Deserialize<'de> for MinMaxBounds<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, bound = "T: Bound")]
        struct Range<T> {
            #[serde(default, deserialize_with = "optional_bound")]
            min: Option<T>,
            #[serde(default, deserialize_with = "optional_bound")]
            max: Option<T>,
        }

        struct BoundsVisitor<T>(PhantomData<T>, bool);

        fn exactly<T: Bound, E: serde::de::Error>(number: Number) -> Result<MinMaxBounds<T>, E> {
            let exact = T::read(number).map_err(E::custom)?;
            Ok(MinMaxBounds {
                min: Some(exact),
                max: Some(exact),
            })
        }

        impl<'de, T: Bound> Visitor<'de> for BoundsVisitor<T> {
            type Value = MinMaxBounds<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a {min, max} range")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let Range::<T> { min, max } =
                    Range::deserialize(value::MapAccessDeserializer::new(map))?;
                if let (Some(lo), Some(hi)) = (min, max)
                    && lo.compare(hi) == Ordering::Greater
                {
                    return Err(A::Error::custom(format_args!(
                        "Swapped bounds in range: Optional[{}] is higher than Optional[{}]",
                        lo.java_string(),
                        hi.java_string()
                    )));
                }
                Ok(MinMaxBounds { min, max })
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                exactly(Number::I64(v, self.1))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                exactly(Number::U64(v, self.1))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                exactly(Number::F64(v, self.1))
            }
        }

        let human_readable = d.is_human_readable();
        d.deserialize_any(BoundsVisitor(PhantomData, human_readable))
    }
}

#[cfg(test)]
mod tests {
    use super::MinMaxBounds;

    #[test]
    fn bounds_write_back_as_read() {
        for json in ["0.9", r#"{"min":0.9}"#, r#"{"min":0.1,"max":0.5}"#, "{}"] {
            let bounds: MinMaxBounds<f64> = serde_json::from_str(json).unwrap();
            assert_eq!(serde_json::to_string(&bounds).unwrap(), json);
        }
        assert!(serde_json::from_str::<MinMaxBounds<f64>>(r#"{"min":2,"max":1}"#).is_err());
    }

    #[test]
    fn a_lower_bound_matches_at_and_above_it() {
        let at_least: MinMaxBounds<f64> = serde_json::from_str(r#"{"min":0.9}"#).unwrap();
        assert!(at_least.matches(1.0) && at_least.matches(0.9) && !at_least.matches(0.8));
    }
}
