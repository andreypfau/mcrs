use core::fmt;

use mcrs_minecraft_loot_condition::IntProviderFile;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Default)]
pub struct KeyedNumbers(pub Vec<KeyedNumber>);

#[derive(Debug, Clone)]
pub struct KeyedNumber {
    pub key: ResourceLocationBuf,
    pub number: IntProviderFile,
}

impl<'de> Deserialize<'de> for KeyedNumbers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(KeyedNumbersVisitor)
    }
}

struct KeyedNumbersVisitor;

impl<'de> Visitor<'de> for KeyedNumbersVisitor {
    type Value = KeyedNumbers;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map of keys to number providers")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<KeyedNumbers, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, number)) = map.next_entry()? {
            entries.push(KeyedNumber { key, number });
        }
        Ok(KeyedNumbers(entries))
    }
}
