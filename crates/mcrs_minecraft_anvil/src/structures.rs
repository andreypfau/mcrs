use mcrs_minecraft_resource::ResourceLocationBuf;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::piece::SavedPiece;
use mcrs_voxel::ColumnPos;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::marker::PhantomData;

/// The structure ids are kept as written: this layer has no structure registry,
/// so an id it does not name is for the caller to reject.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkStructures {
    pub starts: BTreeMap<ResourceLocationBuf, StructureStart>,
    /// The chunks holding a start of the structure that reaches into this one.
    pub references: BTreeMap<ResourceLocationBuf, BTreeSet<ColumnPos>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructureStart {
    pub references: i32,
    /// The saved pieces, each typed by its own `id`; never empty once loaded.
    pub pieces: Vec<SavedPiece>,
}

/// A saved structure entry that did not load. The rest of the chunk loads
/// without it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DroppedStructure {
    StartId(String),
    StartWithoutPieces(ResourceLocationBuf),
    ReferenceId(String),
    FarReference {
        structure: ResourceLocationBuf,
        chunk: ColumnPos,
    },
}

const MAX_REFERENCE_DISTANCE: u32 = 8;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Structures<'a> {
    /// Written as each start's `ChunkX`/`ChunkZ`; the game writes the chunk
    /// holding the start there, so the read value carries nothing more.
    pub(crate) pos: ColumnPos,
    pub(crate) kept: Cow<'a, ChunkStructures>,
    pub(crate) dropped: Vec<DroppedStructure>,
}

impl Structures<'_> {
    pub(crate) fn within(self, column: ColumnPos) -> (ChunkStructures, Vec<DroppedStructure>) {
        let mut kept = self.kept.into_owned();
        let mut dropped = self.dropped;
        for (structure, chunks) in &mut kept.references {
            chunks.retain(|&chunk| {
                let near = chunk.x.abs_diff(column.x).max(chunk.z.abs_diff(column.z))
                    <= MAX_REFERENCE_DISTANCE;
                if !near {
                    dropped.push(DroppedStructure::FarReference {
                        structure: structure.clone(),
                        chunk,
                    });
                }
                near
            });
        }
        kept.references.retain(|_, chunks| !chunks.is_empty());
        (kept, dropped)
    }
}

fn unpack(packed: i64) -> ColumnPos {
    ColumnPos::new(packed as i32, (packed >> 32) as i32)
}

fn pack(pos: ColumnPos) -> i64 {
    (i64::from(pos.x) & 0xFFFF_FFFF) | (i64::from(pos.z) << 32)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadStart {
    #[serde(rename = "ChunkX", default)]
    _chunk_x: i32,
    #[serde(rename = "ChunkZ", default)]
    _chunk_z: i32,
    #[serde(default)]
    references: i32,
    #[serde(rename = "Children", default)]
    children: Vec<SavedPiece>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadStructures {
    #[serde(default)]
    starts: ById<ReadStart>,
    #[serde(rename = "References", default)]
    references: ById<Vec<i64>>,
}

/// A map keyed by structure id whose value is read only when its key is an id, so that an entry
/// dropped for its id cannot fail the column with an error in its value.
struct ById<T> {
    kept: Vec<(ResourceLocationBuf, T)>,
    invalid: Vec<String>,
}

impl<T> Default for ById<T> {
    fn default() -> Self {
        Self {
            kept: Vec::new(),
            invalid: Vec::new(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for ById<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Entries<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for Entries<T> {
            type Value = ById<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a compound keyed by structure id")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = ById::default();
                while let Some(id) = map.next_key::<String>()? {
                    match ResourceLocationBuf::read(&id) {
                        Ok(structure) => entries.kept.push((structure, map.next_value()?)),
                        Err(_) => {
                            map.next_value::<IgnoredAny>()?;
                            entries.invalid.push(id);
                        }
                    }
                }
                Ok(entries)
            }
        }

        d.deserialize_map(Entries(PhantomData))
    }
}

impl<'de> Deserialize<'de> for Structures<'_> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = ReadStructures::deserialize(d)?;
        let mut kept = ChunkStructures::default();
        let mut dropped = Vec::new();
        dropped.extend(
            raw.starts
                .invalid
                .into_iter()
                .map(DroppedStructure::StartId),
        );
        for (structure, start) in raw.starts.kept {
            if start.children.is_empty() {
                dropped.push(DroppedStructure::StartWithoutPieces(structure));
            } else {
                kept.starts.insert(
                    structure,
                    StructureStart {
                        references: start.references,
                        pieces: start.children,
                    },
                );
            }
        }
        dropped.extend(
            raw.references
                .invalid
                .into_iter()
                .map(DroppedStructure::ReferenceId),
        );
        for (structure, chunks) in raw.references.kept {
            kept.references
                .insert(structure, chunks.into_iter().map(unpack).collect());
        }
        Ok(Structures {
            pos: ColumnPos::default(),
            kept: Cow::Owned(kept),
            dropped,
        })
    }
}

#[derive(Serialize)]
struct RawStart<'a> {
    #[serde(rename = "ChunkX")]
    chunk_x: i32,
    #[serde(rename = "ChunkZ")]
    chunk_z: i32,
    references: i32,
    #[serde(rename = "Children")]
    children: &'a [SavedPiece],
}

struct Starts<'a>(&'a Structures<'a>);

impl Serialize for Starts<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let Structures { pos, kept, .. } = self.0;
        s.collect_map(kept.starts.iter().map(|(id, start)| {
            (
                id,
                RawStart {
                    chunk_x: pos.x,
                    chunk_z: pos.z,
                    references: start.references,
                    children: &start.pieces,
                },
            )
        }))
    }
}

struct LongArray(Vec<i64>);

impl Serialize for LongArray {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        mcrs_minecraft_nbt::nbt_long_array(&self.0, s)
    }
}

struct References<'a>(&'a BTreeMap<ResourceLocationBuf, BTreeSet<ColumnPos>>);

impl Serialize for References<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(
            self.0
                .iter()
                .filter(|(_, chunks)| !chunks.is_empty())
                .map(|(id, chunks)| (id, LongArray(chunks.iter().copied().map(pack).collect()))),
        )
    }
}

impl Serialize for Structures<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("starts", &Starts(self))?;
        map.serialize_entry("References", &References(&self.kept.references))?;
        map.end()
    }
}
