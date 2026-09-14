use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
        let value = i32::deserialize(deserializer)?;
        if !(MIN..=MAX).contains(&value) {
            return Err(D::Error::custom(format!(
                "Value must be within range [{MIN};{MAX}]: {value}"
            )));
        }
        Ok(Bounded(value))
    }
}

impl<const MIN: i32, const MAX: i32, const DEFAULT: i32> Serialize for Bounded<MIN, MAX, DEFAULT> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
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
