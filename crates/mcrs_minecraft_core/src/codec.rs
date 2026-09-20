use std::fmt;

use serde::de::{Error as _, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `Codec.INT`: any number's `intValue()`. An integer keeps its low 32 bits;
/// a fraction is dropped, and a value beyond the int range keeps its low 32
/// bits from JSON (`BigDecimal.intValue`) but saturates from NBT
/// (`Double.intValue`).
pub fn int_value<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    struct IntValue {
        wrap_floats: bool,
    }

    impl Visitor<'_> for IntValue {
        type Value = i32;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a number")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<i32, E> {
            Ok(v as i32)
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<i32, E> {
            Ok(v as i32)
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<i32, E> {
            if !self.wrap_floats {
                return Ok(v as i32);
            }
            let truncated = v.trunc();
            Ok(if truncated.abs() >= 2f64.powi(127) {
                0
            } else {
                truncated as i128 as i32
            })
        }
    }

    let wrap_floats = d.is_human_readable();
    d.deserialize_any(IntValue { wrap_floats })
}

/// `Codec.FLOAT`: any number's `floatValue()`.
pub fn float_value<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    struct FloatValue;

    impl Visitor<'_> for FloatValue {
        type Value = f32;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a number")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<f32, E> {
            Ok(v as f32)
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<f32, E> {
            Ok(v as f32)
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<f32, E> {
            Ok(v as f32)
        }
    }

    d.deserialize_any(FloatValue)
}

/// A `Codec.intRange(MIN, MAX)` payload, with the value `optionalFieldOf`
/// falls back to. Stated once here rather than as a validator per field,
/// because the tree registries carry a dozen distinct bounds across sixty-odd
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bounded<const MIN: i32, const MAX: i32, const DEFAULT: i32 = 0>(pub i32);

impl<const MIN: i32, const MAX: i32, const DEFAULT: i32> Default for Bounded<MIN, MAX, DEFAULT> {
    fn default() -> Self {
        Bounded(DEFAULT)
    }
}

impl<const MIN: i32, const MAX: i32, const DEFAULT: i32> Bounded<MIN, MAX, DEFAULT> {
    fn out_of_range(value: i32) -> String {
        match (MIN, MAX) {
            (0, i32::MAX) => format!("Value must be non-negative: {value}"),
            (1, i32::MAX) => format!("Value must be positive: {value}"),
            _ => format!("Value must be within range [{MIN};{MAX}]: {value}"),
        }
    }
}

pub fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

pub fn default_true() -> bool {
    true
}

impl<'de, const MIN: i32, const MAX: i32, const DEFAULT: i32> Deserialize<'de>
    for Bounded<MIN, MAX, DEFAULT>
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = int_value(deserializer)?;
        if !(MIN..=MAX).contains(&value) {
            return Err(D::Error::custom(Self::out_of_range(value)));
        }
        Ok(Bounded(value))
    }
}

impl<const MIN: i32, const MAX: i32, const DEFAULT: i32> Serialize for Bounded<MIN, MAX, DEFAULT> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !(MIN..=MAX).contains(&self.0) {
            return Err(serde::ser::Error::custom(Self::out_of_range(self.0)));
        }
        self.0.serialize(serializer)
    }
}

pub type NonNegativeInt = Bounded<0, { i32::MAX }>;
pub type PositiveInt = Bounded<1, { i32::MAX }>;

/// A codec bound the field list alone does not express. The shape is derived as
/// usual and `validated!` hangs the check on the way in, so the fields are
/// spelled once rather than once more in a shadow struct that has to be kept in
/// step by hand.
pub trait Validate: Sized {
    fn validate(&self) -> Result<(), String>;
}

/// Turns the inherent codec `#[serde(remote = "Self")]` generates back into the
/// trait impls, checking [`Validate`] on the way in.
#[macro_export]
macro_rules! validated {
    ($($name:ident),* $(,)?) => {$(
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let value = $name::deserialize(deserializer)?;
                value.validate().map_err(serde::de::Error::custom)?;
                Ok(value)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                $name::serialize(self, serializer)
            }
        }
    )*};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_value_reads_any_number_as_java_does() {
        let read = |json: &str| serde_json::from_str::<NonNegativeInt>(json);
        assert_eq!(read("1.5").unwrap().0, 1);
        assert_eq!(read("2.9").unwrap().0, 2);
        assert_eq!(read("1e10").unwrap().0, 1410065408);
        assert_eq!(read("4294967297").unwrap().0, 1);
        assert_eq!(
            read("3000000000.0").unwrap_err().to_string(),
            "Value must be non-negative: -1294967296"
        );
        assert_eq!(read("1e300").unwrap().0, 0);
    }
}
