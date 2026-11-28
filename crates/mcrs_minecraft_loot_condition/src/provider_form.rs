use core::fmt;
use core::marker::PhantomData;

use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone)]
pub(crate) enum ProviderForm<C, T> {
    Constant(C),
    Key(ResourceLocationBuf),
    Inline(Box<T>),
}

pub(crate) trait Number: Sized {
    const EXPECTING: &'static str;

    fn from_i64(value: i64) -> Option<Self>;

    fn from_u64(value: u64) -> Option<Self>;

    fn from_f64(value: f64) -> Option<Self>;
}

impl Number for i32 {
    const EXPECTING: &'static str = "an integer in the i32 range";

    fn from_i64(value: i64) -> Option<Self> {
        i32::try_from(value).ok()
    }

    fn from_u64(value: u64) -> Option<Self> {
        i32::try_from(value).ok()
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is integral and in the i32 range"
    )]
    fn from_f64(value: f64) -> Option<Self> {
        let integral =
            value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX);
        integral.then_some(value as i32)
    }
}

impl Number for f32 {
    const EXPECTING: &'static str = "a number";

    #[expect(clippy::cast_precision_loss, reason = "the nearest f32 of the number")]
    fn from_i64(value: i64) -> Option<Self> {
        Some(value as f32)
    }

    #[expect(clippy::cast_precision_loss, reason = "the nearest f32 of the number")]
    fn from_u64(value: u64) -> Option<Self> {
        Some(value as f32)
    }

    #[expect(clippy::cast_possible_truncation, reason = "the narrow of the number")]
    fn from_f64(value: f64) -> Option<Self> {
        Some(value as f32)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NumberValue<C>(pub(crate) C);

impl<'de, C: Number> Deserialize<'de> for NumberValue<C> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(FormVisitor::<C, Never>(PhantomData))
            .and_then(|form| match form {
                ProviderForm::Constant(value) => Ok(NumberValue(value)),
                ProviderForm::Key(_) | ProviderForm::Inline(_) => {
                    Err(de::Error::custom(C::EXPECTING))
                }
            })
    }
}

#[derive(Debug)]
enum Never {}

impl<'de> Deserialize<'de> for Never {
    fn deserialize<D: Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        Err(de::Error::custom("a number"))
    }
}

impl<'de, C: Number, T: Deserialize<'de>> Deserialize<'de> for ProviderForm<C, T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(FormVisitor(PhantomData))
    }
}

struct FormVisitor<C, T>(PhantomData<(C, T)>);

impl<C: Number, T> FormVisitor<C, T> {
    fn constant<E: de::Error>(
        value: Option<C>,
        shown: de::Unexpected<'_>,
    ) -> Result<ProviderForm<C, T>, E> {
        value
            .map(ProviderForm::Constant)
            .ok_or_else(|| E::invalid_value(shown, &C::EXPECTING))
    }
}

impl<'de, C: Number, T: Deserialize<'de>> Visitor<'de> for FormVisitor<C, T> {
    type Value = ProviderForm<C, T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a number, the key of a provider or a provider object")
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Self::constant(C::from_i64(value), de::Unexpected::Signed(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Self::constant(C::from_u64(value), de::Unexpected::Unsigned(value))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Self::constant(C::from_f64(value), de::Unexpected::Float(value))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
        ResourceLocationBuf::read(text)
            .map(ProviderForm::Key)
            .map_err(E::custom)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(de::value::MapAccessDeserializer::new(map))
            .map(|typed| ProviderForm::Inline(Box::new(typed)))
    }
}
