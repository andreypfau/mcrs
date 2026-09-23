use mcrs_minecraft_core::ColumnPos;
use std::collections::BTreeMap;
use std::hash::Hash;
use std::io::Cursor;

use mcrs_minecraft_chunk::section::{Biomes, Blocks};
use mcrs_minecraft_chunk::{PalettedContainer, SectionKind, VoxelId};
use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::Deserialize;

use crate::palette::{BlockStateList, PaletteLookup};
use crate::{DATA_VERSION, ErrorKind, accepts_data_version};

pub const LIGHT_BYTES: usize = 2048;

/// One nibble per cell, indexed the same way block states are.
pub type Light = mcrs_minecraft_chunk::SectionNibbles;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub y: i8,
    pub block_states: Option<PalettedContainer<VoxelId, { Blocks::SIZE }>>,
    pub biomes: Option<PalettedContainer<u8, { Biomes::SIZE }>>,
    pub block_light: Option<Light>,
    pub sky_light: Option<Light>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub pos: ColumnPos,
    /// `yPos`: the section Y the saved section array starts at.
    pub min_section_y: i32,
    pub status: String,
    pub is_light_on: bool,
    pub inhabited_time: i64,
    pub last_update: i64,
    pub heightmaps: BTreeMap<String, Vec<i64>>,
    pub block_entities: Vec<NbtCompound>,
    pub sections: Vec<Section>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPalettedContainer {
    palette: BlockStateList,
    data: Option<PackedData>,
}

#[derive(Deserialize)]
struct RawSection {
    #[serde(rename = "Y")]
    y: i8,
    block_states: Option<RawPalettedContainer>,
    biomes: Option<RawPalettedContainer>,
    #[serde(rename = "BlockLight")]
    block_light: Option<RawLight>,
    #[serde(rename = "SkyLight")]
    sky_light: Option<RawLight>,
}

#[derive(Deserialize)]
struct RawChunk {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "xPos")]
    x_pos: i32,
    #[serde(rename = "zPos")]
    z_pos: i32,
    #[serde(rename = "yPos")]
    y_pos: i32,
    #[serde(rename = "Status")]
    status: String,
    #[serde(default)]
    sections: Vec<RawSection>,
    #[serde(rename = "Heightmaps", default)]
    heightmaps: BTreeMap<String, Vec<i64>>,
    #[serde(rename = "isLightOn", default)]
    is_light_on: bool,
    #[serde(default)]
    block_entities: Vec<NbtCompound>,
    #[serde(rename = "InhabitedTime", default)]
    inhabited_time: i64,
    #[serde(rename = "LastUpdate", default)]
    last_update: i64,
}

#[derive(Deserialize)]
struct RawChunkVersion {
    #[serde(rename = "DataVersion", default)]
    data_version: Option<i32>,
}

/// A chunk written by an older version fails on a field this layout never had,
/// which says nothing useful. Re-read just the version so the error names it.
fn wrong_version(nbt: &[u8]) -> Option<ErrorKind> {
    let raw: RawChunkVersion = mcrs_minecraft_nbt::from_bytes(Cursor::new(nbt)).ok()?;
    match raw.data_version {
        None => Some(ErrorKind::MissingDataVersion {
            expected: DATA_VERSION,
        }),
        Some(found) if !accepts_data_version(found) => Some(ErrorKind::DataVersion {
            found,
            expected: DATA_VERSION,
        }),
        Some(_) => None,
    }
}

pub fn parse(
    nbt: &[u8],
    blocks: &impl PaletteLookup<VoxelId>,
    biomes: &impl PaletteLookup<u8>,
) -> Result<Chunk, ErrorKind> {
    let raw: RawChunk = match mcrs_minecraft_nbt::from_bytes(Cursor::new(nbt)) {
        Ok(raw) => raw,
        Err(err) => return Err(wrong_version(nbt).unwrap_or_else(|| err.into())),
    };
    if !accepts_data_version(raw.data_version) {
        return Err(ErrorKind::DataVersion {
            found: raw.data_version,
            expected: DATA_VERSION,
        });
    }
    Ok(Chunk {
        pos: ColumnPos::new(raw.x_pos, raw.z_pos),
        min_section_y: raw.y_pos,
        status: raw.status,
        is_light_on: raw.is_light_on,
        inhabited_time: raw.inhabited_time,
        last_update: raw.last_update,
        heightmaps: raw.heightmaps,
        block_entities: raw.block_entities,
        sections: raw
            .sections
            .into_iter()
            .map(|section| parse_section(section, blocks, biomes))
            .collect::<Result<_, _>>()?,
    })
}

fn parse_section(
    raw: RawSection,
    blocks: &impl PaletteLookup<VoxelId>,
    biomes: &impl PaletteLookup<u8>,
) -> Result<Section, ErrorKind> {
    let y = raw.y;
    Ok(Section {
        y,
        block_states: raw
            .block_states
            .map(|c| unpack::<Blocks, _, _>(c, blocks, y, "block_states"))
            .transpose()?,
        biomes: raw
            .biomes
            .map(|c| unpack::<Biomes, _, _>(c, biomes, y, "biomes"))
            .transpose()?,
        block_light: raw
            .block_light
            .map(|bytes| light(bytes.0, y, "BlockLight"))
            .transpose()?,
        sky_light: raw
            .sky_light
            .map(|bytes| light(bytes.0, y, "SkyLight"))
            .transpose()?,
    })
}

/// Cells are indices into the palette list, in `Strategy.getIndex` order, at the
/// width the list's length selects. The list may name entries no cell uses.
fn unpack<K: SectionKind, V: Hash + Eq + Copy + Default, const DIM: usize>(
    raw: RawPalettedContainer,
    lookup: &impl PaletteLookup<V>,
    y: i8,
    field: &'static str,
) -> Result<PalettedContainer<V, DIM>, ErrorKind> {
    const { assert!(DIM == K::SIZE) };
    let len = raw.palette.len();
    if len == 0 {
        return Err(ErrorKind::EmptyPalette { y, field });
    }
    if len > u16::MAX as usize + 1 {
        return Err(ErrorKind::PaletteTooLarge {
            y,
            field,
            len,
            max: u16::MAX as usize + 1,
        });
    }
    let bits = K::storage_bits(len);
    if bits == 0 {
        if raw.data.is_some() {
            return Err(ErrorKind::UnexpectedData { y, field });
        }
        return Ok(PalettedContainer::Homogeneous(resolve(
            &raw.palette,
            lookup,
            0,
        )?));
    }

    let Some(data) = raw.data else {
        return Err(ErrorKind::MissingData { y, field, bits });
    };
    let data = data.0;
    mcrs_minecraft_chunk::check_len(bits, &data, K::ENTRY_COUNT).map_err(|e| {
        ErrorKind::DataLength {
            y,
            field,
            found: e.found,
            expected: e.expected,
            bits,
        }
    })?;

    if mcrs_minecraft_chunk::any_entry_past(bits, &data, K::ENTRY_COUNT, len) {
        let index = mcrs_minecraft_chunk::first_entry_past(bits, &data, K::ENTRY_COUNT, len)
            .expect("the maximum is already past the palette");
        return Err(ErrorKind::PaletteIndex {
            y,
            field,
            index,
            len,
        });
    }

    let entries = (0..len)
        .map(|index| resolve(&raw.palette, lookup, index))
        .collect::<Result<Vec<V>, _>>()?;
    let mut cells = vec![V::default(); K::ENTRY_COUNT];
    mcrs_minecraft_chunk::remap_into(bits, &data, &entries, &mut cells)
        .expect("the data length was checked above");
    Ok(PalettedContainer::from_cells(&cells))
}

/// Resolved while the name is still borrowed out of the palette's text.
fn resolve<V>(
    palette: &BlockStateList,
    lookup: &impl PaletteLookup<V>,
    index: usize,
) -> Result<V, ErrorKind> {
    let name = palette.name(index);
    lookup
        .resolve(name, palette.properties(index))
        .ok_or_else(|| ErrorKind::UnknownPaletteEntry {
            name: name.to_string(),
        })
}

/// Asks the deserializer for the long array whole; read as a sequence it costs
/// a visitor round trip per element.
struct PackedData(Box<[i64]>);

impl<'de> Deserialize<'de> for PackedData {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Longs;

        impl<'de> serde::de::Visitor<'de> for Longs {
            type Value = PackedData;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a long array")
            }

            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<PackedData, E> {
                if !v.len().is_multiple_of(8) {
                    return Err(E::invalid_length(v.len(), &self));
                }
                Ok(PackedData(
                    v.chunks_exact(8)
                        .map(|word| i64::from_be_bytes(word.try_into().unwrap()))
                        .collect(),
                ))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<PackedData, A::Error> {
                let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(word) = seq.next_element::<i64>()? {
                    out.push(word);
                }
                Ok(PackedData(out.into_boxed_slice()))
            }
        }

        d.deserialize_bytes(Longs)
    }
}

/// Asks the deserializer for the byte array whole. Reading it as a sequence
/// costs a visitor round trip per byte, and every section carries two of them.
struct RawLight(Vec<u8>);

impl<'de> Deserialize<'de> for RawLight {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Bytes;

        impl<'de> serde::de::Visitor<'de> for Bytes {
            type Value = RawLight;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a byte array")
            }

            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<RawLight, E> {
                Ok(RawLight(v.to_vec()))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<RawLight, A::Error> {
                let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(LIGHT_BYTES));
                while let Some(byte) = seq.next_element::<u8>()? {
                    out.push(byte);
                }
                Ok(RawLight(out))
            }
        }

        d.deserialize_bytes(Bytes)
    }
}

fn light(bytes: Vec<u8>, y: i8, field: &'static str) -> Result<Light, ErrorKind> {
    let found = bytes.len();
    let bytes: Box<[u8; LIGHT_BYTES]> = bytes
        .into_boxed_slice()
        .try_into()
        .map_err(|_| ErrorKind::LightLength { y, field, found })?;
    Ok(mcrs_minecraft_chunk::SectionNibbles(bytes))
}
