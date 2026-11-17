use serde::de;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ListOperation {
    ReplaceAll,
    ReplaceSection {
        #[serde(default)]
        offset: i32,
        size: Option<i32>,
    },
    Insert {
        #[serde(default)]
        offset: i32,
    },
    Append,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct StandaloneList<T, const MAX: usize = { usize::MAX }> {
    #[serde(deserialize_with = "at_most::<_, _, MAX>")]
    pub values: Vec<T>,
    #[serde(flatten)]
    pub operation: ListOperation,
}

pub(crate) fn at_most<'de, D: Deserializer<'de>, T: Deserialize<'de>, const MAX: usize>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    let values = Vec::deserialize(deserializer)?;
    match values.len() <= MAX {
        true => Ok(values),
        false => Err(de::Error::invalid_length(values.len(), &"fewer values")),
    }
}

pub(crate) fn not_empty<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    let values = Vec::deserialize(deserializer)?;
    match values.is_empty() {
        true => Err(de::Error::invalid_length(0, &"at least one value")),
        false => Ok(values),
    }
}
