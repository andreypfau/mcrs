use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, Serialize};

use crate::ChunkStatus;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct RetroGen {
    pub target_status: ChunkStatus,
    pub statuses_to_rerun: Vec<ChunkStatus>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_below_zero_retrogen: bool,
    #[serde(
        default,
        deserialize_with = "missing_bedrock",
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "mcrs_minecraft_nbt::nbt_long_array"
    )]
    pub missing_bedrock: Vec<i64>,
}

mcrs_minecraft_core::validated!(RetroGen);

impl Validate for RetroGen {
    fn validate(&self) -> Result<(), String> {
        if self.target_status == ChunkStatus::Empty {
            return Err("target_status cannot be empty".to_owned());
        }
        if self.statuses_to_rerun.contains(&ChunkStatus::Empty) {
            return Err("statuses_to_rerun cannot be empty".to_owned());
        }
        Ok(())
    }
}

fn missing_bedrock<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i64>, D::Error> {
    let mut words = Words::deserialize(deserializer)?.0.unwrap_or_default();
    while words.last() == Some(&0) {
        words.pop();
    }
    Ok(words)
}

/// A list or an array of any element width reads number by number, one word
/// per element. A value holding anything else reads as absent, consumed so the
/// rest of the record still loads; every other field of the record is strict.
pub(crate) struct Words(pub(crate) Option<Vec<i64>>);

impl<'de> Deserialize<'de> for Words {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // A float element narrows as a cast does: toward zero, saturating, and
        // not floored the way a float tag read as an integer field is.
        let word = |tag: &NbtTag| match *tag {
            NbtTag::Byte(v) => Some(i64::from(v)),
            NbtTag::Short(v) => Some(i64::from(v)),
            NbtTag::Int(v) => Some(i64::from(v)),
            NbtTag::Long(v) => Some(v),
            NbtTag::Float(v) => Some(v as i64),
            NbtTag::Double(v) => Some(v as i64),
            _ => None,
        };
        Ok(Words(
            match <NbtTag as Deserialize>::deserialize(deserializer)? {
                NbtTag::ByteArray(bytes) => {
                    Some(bytes.iter().map(|&b| i64::from(b as i8)).collect())
                }
                NbtTag::IntArray(ints) => Some(ints.into_iter().map(i64::from).collect()),
                NbtTag::LongArray(longs) => Some(longs),
                NbtTag::List(tags) => tags.iter().map(word).collect(),
                _ => None,
            },
        ))
    }
}
