use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_voxel::ColumnPos;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The structure ids are kept as written: this layer has no structure registry,
/// so an id it does not name is for the caller to reject.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkStructures {
    pub starts: BTreeMap<ResourceLocation, StructureStart>,
    /// The chunks holding a start of the structure that reaches into this one.
    pub references: BTreeMap<ResourceLocation, BTreeSet<ColumnPos>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructureStart {
    pub references: i32,
    /// The saved pieces, each typed by its own `id`; never empty once loaded.
    pub pieces: Vec<NbtCompound>,
}

/// A saved structure entry that did not load. The rest of the chunk loads
/// without it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DroppedStructure {
    StartId(String),
    StartWithoutPieces(ResourceLocation),
    ReferenceId(String),
    FarReference {
        structure: ResourceLocation,
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

fn compound(tag: NbtTag) -> NbtCompound {
    match tag {
        NbtTag::Compound(compound) => compound,
        _ => NbtCompound::new(),
    }
}

fn start(tag: NbtTag) -> StructureStart {
    let mut start = StructureStart {
        references: 0,
        pieces: Vec::new(),
    };
    for (key, value) in compound(tag).child_tags {
        match (key.as_str(), value) {
            ("references", NbtTag::Int(references)) => start.references = references,
            ("Children", NbtTag::List(children)) => {
                start.pieces = children
                    .into_iter()
                    .filter_map(|piece| match piece {
                        NbtTag::Compound(piece) => Some(piece),
                        _ => None,
                    })
                    .collect();
            }
            _ => {}
        }
    }
    start
}

fn decode(tag: NbtTag) -> Structures<'static> {
    let mut kept = ChunkStructures::default();
    let mut dropped = Vec::new();
    for (key, value) in compound(tag).child_tags {
        match key.as_str() {
            "starts" => {
                for (id, value) in compound(value).child_tags {
                    let Ok(structure) = ResourceLocation::read(&id) else {
                        dropped.push(DroppedStructure::StartId(id));
                        continue;
                    };
                    let start = start(value);
                    if start.pieces.is_empty() {
                        dropped.push(DroppedStructure::StartWithoutPieces(structure));
                    } else {
                        kept.starts.insert(structure, start);
                    }
                }
            }
            "References" => {
                for (id, value) in compound(value).child_tags {
                    let Ok(structure) = ResourceLocation::read(&id) else {
                        dropped.push(DroppedStructure::ReferenceId(id));
                        continue;
                    };
                    if let NbtTag::LongArray(chunks) = value {
                        kept.references
                            .insert(structure, chunks.into_iter().map(unpack).collect());
                    }
                }
            }
            _ => {}
        }
    }
    Structures {
        pos: ColumnPos::default(),
        kept: Cow::Owned(kept),
        dropped,
    }
}

impl<'de> Deserialize<'de> for Structures<'_> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        <NbtTag as Deserialize>::deserialize(d).map(decode)
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
    children: &'a [NbtCompound],
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

struct References<'a>(&'a BTreeMap<ResourceLocation, BTreeSet<ColumnPos>>);

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
