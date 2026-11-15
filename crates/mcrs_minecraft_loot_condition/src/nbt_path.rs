use mcrs_minecraft_nbt_path::NbtPath;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

/// The `path` of `storage` (REG-FLD-734); a path that the grammar does not accept fails the
/// decode.
pub(crate) fn nbt_path<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NbtPath, D::Error> {
    let text = String::deserialize(deserializer)?;
    text.parse().map_err(D::Error::custom)
}
