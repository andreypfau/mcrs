use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_registry::Holder;
use mcrs_minecraft_registry::dispatch::Buffered;
use serde::de::value::MapDeserializer;
use serde::de::{DeserializeOwned, DeserializeSeed, IntoDeserializer, MapAccess};

/// A map read whole, by key.
pub(crate) struct MapEntries(Vec<(String, Buffered)>);

impl MapEntries {
    pub(crate) fn read<'de, A: MapAccess<'de>>(mut map: A) -> Result<Self, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry::<String, Buffered>()? {
            entries.push(entry);
        }
        Ok(MapEntries(entries))
    }

    pub(crate) fn has(&self, key: &str) -> bool {
        self.0.iter().any(|(name, _)| name == key)
    }

    pub(crate) fn take(&mut self, key: &str) -> Option<Buffered> {
        let index = self.0.iter().position(|(name, _)| name == key)?;
        Some(self.0.remove(index).1)
    }

    pub(crate) fn into_value<T: DeserializeOwned, E: serde::de::Error>(self) -> Result<T, E> {
        T::deserialize(MapDeserializer::new(self.0.into_iter()))
    }

    pub(crate) fn into_holder<V, E>(self) -> Result<Holder<V>, E>
    where
        V: RegistryValue + DeserializeOwned,
        E: serde::de::Error,
    {
        self.into_value()
    }
}

/// Reads one buffered value with a seed chosen after the rest of its map.
pub(crate) fn read_seeded<'de, S, E>(seed: S, value: Buffered) -> Result<S::Value, E>
where
    S: DeserializeSeed<'de>,
    E: serde::de::Error,
{
    seed.deserialize(value.into_deserializer())
}
