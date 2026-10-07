use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::keys::DataComponentType;
use mcrs_minecraft_registry::Id;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::slots::SlotRange;

/// A map kept in the order read, refusing a repeated key.
#[derive(Debug, Clone, PartialEq)]
pub struct UniqueMap<K, V>(pub Vec<(K, V)>);

impl<K, V> Default for UniqueMap<K, V> {
    fn default() -> Self {
        UniqueMap(Vec::new())
    }
}

impl<K, V> UniqueMap<K, V> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub trait UniqueKey: PartialEq {
    fn duplicate_error(&self) -> String;
}

impl UniqueKey for String {
    fn duplicate_error(&self) -> String {
        format!("Duplicate key '{self}'")
    }
}

impl UniqueKey for SlotRange {
    fn duplicate_error(&self) -> String {
        format!("Duplicate key '{}'", self.name())
    }
}

impl UniqueKey for DataComponentType {
    fn duplicate_error(&self) -> String {
        format!("Duplicate key '{}'", self.as_static_str())
    }
}

impl UniqueKey for Id<EnchantmentData> {
    fn duplicate_error(&self) -> String {
        "Duplicate enchantment".to_owned()
    }
}

impl<K: Serialize, V: Serialize> Serialize for UniqueMap<K, V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.iter().map(|(key, value)| (key, value)))
    }
}

impl<'de, K, V> Deserialize<'de> for UniqueMap<K, V>
where
    K: Deserialize<'de> + UniqueKey,
    V: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct UniqueVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K, V> Visitor<'de> for UniqueVisitor<K, V>
        where
            K: Deserialize<'de> + UniqueKey,
            V: Deserialize<'de>,
        {
            type Value = UniqueMap<K, V>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries: Vec<(K, V)> = Vec::new();
                while let Some(key) = map.next_key::<K>()? {
                    if entries.iter().any(|(seen, _)| *seen == key) {
                        return Err(A::Error::custom(key.duplicate_error()));
                    }
                    entries.push((key, map.next_value()?));
                }
                Ok(UniqueMap(entries))
            }
        }

        d.deserialize_map(UniqueVisitor(PhantomData))
    }
}
