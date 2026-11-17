use mcrs_minecraft_nbt_path::NbtPath;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

pub(crate) fn nbt_path<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NbtPath, D::Error> {
    let text = String::deserialize(deserializer)?;
    text.parse().map_err(D::Error::custom)
}
