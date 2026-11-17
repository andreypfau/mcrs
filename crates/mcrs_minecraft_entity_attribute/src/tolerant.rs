use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use core::str::FromStr;

use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::attribute_keys::Attribute;
use crate::modifier_operation::ModifierOperation;

pub(crate) trait Tolerant: Sized {
    fn from_number(_number: f64) -> Option<Self> {
        None
    }

    fn from_text(_text: &str) -> Option<Self> {
        None
    }

    fn from_compound<'de, M: MapAccess<'de>>(mut map: M) -> Result<Option<Self>, M::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }

    fn from_list<'de, S: SeqAccess<'de>>(mut seq: S) -> Result<Option<Self>, S::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

pub(crate) struct Lenient<T>(pub(crate) Option<T>);

impl<'de, T: Tolerant> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(LenientVisitor(PhantomData))
            .map(Lenient)
    }
}

struct LenientVisitor<T>(PhantomData<T>);

impl<'de, T: Tolerant> Visitor<'de> for LenientVisitor<T> {
    type Value = Option<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_i64<E>(self, number: i64) -> Result<Self::Value, E> {
        Ok(T::from_number(number as f64))
    }

    fn visit_u64<E>(self, number: u64) -> Result<Self::Value, E> {
        Ok(T::from_number(number as f64))
    }

    fn visit_f64<E>(self, number: f64) -> Result<Self::Value, E> {
        Ok(T::from_number(number))
    }

    fn visit_str<E>(self, text: &str) -> Result<Self::Value, E> {
        Ok(T::from_text(text))
    }

    fn visit_bytes<E>(self, _: &[u8]) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        Lenient::<T>::deserialize(deserializer).map(|lenient| lenient.0)
    }

    fn visit_seq<S: SeqAccess<'de>>(self, seq: S) -> Result<Self::Value, S::Error> {
        T::from_list(seq)
    }

    fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
        T::from_compound(map)
    }
}

impl Tolerant for f64 {
    fn from_number(number: f64) -> Option<Self> {
        Some(number)
    }
}

impl Tolerant for Attribute {
    fn from_text(text: &str) -> Option<Self> {
        Attribute::read(text)
    }
}

impl Tolerant for ResourceLocationBuf {
    fn from_text(text: &str) -> Option<Self> {
        ResourceLocationBuf::read(text).ok()
    }
}

impl Tolerant for ModifierOperation {
    fn from_text(text: &str) -> Option<Self> {
        ModifierOperation::from_str(text).ok()
    }
}

impl<T: Tolerant> Tolerant for Vec<T> {
    fn from_list<'de, S: SeqAccess<'de>>(mut seq: S) -> Result<Option<Self>, S::Error> {
        let mut elements = Vec::new();
        while let Some(Lenient(element)) = seq.next_element::<Lenient<T>>()? {
            elements.extend(element);
        }
        Ok(Some(elements))
    }
}
