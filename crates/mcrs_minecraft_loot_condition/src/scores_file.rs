use core::fmt;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::int_provider_file::IntProviderFile;
use crate::range_file::RangeFile;

#[derive(Debug, Clone)]
pub(crate) struct ScoresFile(pub(crate) Vec<ObjectiveFile>);

#[derive(Debug, Clone)]
pub(crate) struct ObjectiveFile {
    pub(crate) objective: String,
    pub(crate) range: RangeFile<IntProviderFile>,
}

impl<'de> Deserialize<'de> for ScoresFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;

        impl<'de> Visitor<'de> for Entries {
            type Value = ScoresFile;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map from an objective name to an int range")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ScoresFile, A::Error> {
                let mut entries = Vec::new();
                while let Some((objective, range)) = map.next_entry()? {
                    entries.push(ObjectiveFile { objective, range });
                }
                Ok(ScoresFile(entries))
            }
        }

        deserializer.deserialize_map(Entries)
    }
}
