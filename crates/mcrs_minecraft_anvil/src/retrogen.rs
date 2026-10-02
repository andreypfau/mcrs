use serde::de::{Error as _, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    missing_bedrock: MissingBedrock,
}

fn non_empty(status: ChunkStatus, field: &'static str) -> Result<ChunkStatus, String> {
    match status {
        ChunkStatus::Empty => Err(format!("{field} cannot be empty")),
        status => Ok(status),
    }
}

impl<'de> Deserialize<'de> for RetroGen {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = Stored::deserialize(deserializer)?;
        let target_status =
            non_empty(stored.target_status, "target_status").map_err(D::Error::custom)?;
        for status in &stored.statuses_to_rerun {
            non_empty(*status, "statuses_to_rerun").map_err(D::Error::custom)?;
        }
        let mut missing_bedrock = stored.missing_bedrock.0;
        while missing_bedrock.last() == Some(&0) {
            missing_bedrock.pop();
        }
        Ok(RetroGen {
            target_status,
            statuses_to_rerun: stored.statuses_to_rerun,
            has_below_zero_retrogen: stored.has_below_zero_retrogen,
            missing_bedrock,
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

/// A value that is not a long array reads as absent, consumed so the rest of
/// the record still loads; every other field of the record is strict.
#[derive(Default)]
struct MissingBedrock(Vec<i64>);

impl<'de> Deserialize<'de> for MissingBedrock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_bytes(Words)
    }
}

struct Words;

impl<'de> Visitor<'de> for Words {
    type Value = MissingBedrock;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a long array")
    }

    fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<MissingBedrock, E> {
        if !v.len().is_multiple_of(8) {
            return Ok(MissingBedrock::default());
        }
        Ok(MissingBedrock(
            v.chunks_exact(8)
                .map(|word| i64::from_be_bytes(word.try_into().unwrap()))
                .collect(),
        ))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<MissingBedrock, A::Error> {
        let mut words = Vec::new();
        let mut longs_only = true;
        while let Some(Word(word)) = seq.next_element()? {
            match word {
                Some(word) => words.push(word),
                None => longs_only = false,
            }
        }
        Ok(MissingBedrock(if longs_only { words } else { Vec::new() }))
    }

    fn visit_bool<E>(self, _: bool) -> Result<MissingBedrock, E> {
        Ok(MissingBedrock::default())
    }

    fn visit_i64<E>(self, _: i64) -> Result<MissingBedrock, E> {
        Ok(MissingBedrock::default())
    }

    fn visit_u64<E>(self, _: u64) -> Result<MissingBedrock, E> {
        Ok(MissingBedrock::default())
    }

    fn visit_f64<E>(self, _: f64) -> Result<MissingBedrock, E> {
        Ok(MissingBedrock::default())
    }

    fn visit_str<E>(self, _: &str) -> Result<MissingBedrock, E> {
        Ok(MissingBedrock::default())
    }

    fn visit_unit<E>(self) -> Result<MissingBedrock, E> {
        Ok(MissingBedrock::default())
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<MissingBedrock, A::Error> {
        IgnoredAny.visit_map(map)?;
        Ok(MissingBedrock::default())
    }
}

struct Word(Option<i64>);

impl<'de> Deserialize<'de> for Word {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(WordVisitor)
    }
}

struct WordVisitor;

impl<'de> Visitor<'de> for WordVisitor {
    type Value = Word;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a long")
    }

    fn visit_i64<E>(self, v: i64) -> Result<Word, E> {
        Ok(Word(Some(v)))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Word, E> {
        Ok(Word(Some(v as i64)))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Word, E> {
        Ok(Word(None))
    }

    fn visit_f64<E>(self, _: f64) -> Result<Word, E> {
        Ok(Word(None))
    }

    fn visit_str<E>(self, _: &str) -> Result<Word, E> {
        Ok(Word(None))
    }

    fn visit_unit<E>(self) -> Result<Word, E> {
        Ok(Word(None))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Word, A::Error> {
        IgnoredAny.visit_seq(seq)?;
        Ok(Word(None))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Word, A::Error> {
        IgnoredAny.visit_map(map)?;
        Ok(Word(None))
    }
}
