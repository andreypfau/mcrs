use mcrs_minecraft_registry::dispatch::Buffered;
use serde::de::value::MapDeserializer;
use serde::de::{DeserializeOwned, MapAccess};

/// A map read whole, by key.
pub(crate) struct MapEntries(pub(crate) Vec<(String, Buffered)>);

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
}
