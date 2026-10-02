use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::chunk::PackedData;
use crate::status::ChunkStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetroGen {
    pub target_status: ChunkStatus,
    pub statuses_to_rerun: Vec<ChunkStatus>,
    pub has_below_zero_retrogen: bool,
    pub missing_bedrock: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    target_status: ChunkStatus,
    statuses_to_rerun: Vec<ChunkStatus>,
    #[serde(default)]
    has_below_zero_retrogen: bool,
    #[serde(default)]
    missing_bedrock: Option<PackedData>,
}

impl<'de> Deserialize<'de> for RetroGen {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = Stored::deserialize(deserializer)?;
        Ok(RetroGen {
            target_status: stored.target_status,
            statuses_to_rerun: stored.statuses_to_rerun,
            has_below_zero_retrogen: stored.has_below_zero_retrogen,
            missing_bedrock: stored
                .missing_bedrock
                .map_or_else(Vec::new, |p| p.0.into_vec()),
        })
    }
}

#[derive(Serialize)]
struct Written<'a> {
    target_status: ChunkStatus,
    statuses_to_rerun: &'a [ChunkStatus],
    #[serde(skip_serializing_if = "is_false")]
    has_below_zero_retrogen: bool,
    #[serde(
        skip_serializing_if = "<[i64]>::is_empty",
        serialize_with = "long_array"
    )]
    missing_bedrock: &'a [i64],
}

fn is_false(flag: &bool) -> bool {
    !*flag
}

fn long_array<S: Serializer>(words: &&[i64], serializer: S) -> Result<S::Ok, S::Error> {
    mcrs_minecraft_nbt::nbt_long_array(*words, serializer)
}

impl Serialize for RetroGen {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Written {
            target_status: self.target_status,
            statuses_to_rerun: &self.statuses_to_rerun,
            has_below_zero_retrogen: self.has_below_zero_retrogen,
            missing_bedrock: &self.missing_bedrock,
        }
        .serialize(serializer)
    }
}
