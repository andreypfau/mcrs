use mcrs_minecraft_core::{BlockPos, ColumnPos, VERSION};
use std::collections::BTreeMap;
use std::hash::Hash;
use std::io::Cursor;

use mcrs_minecraft_chunk::section::{Biomes, Blocks, NoiseBiomes};
use mcrs_minecraft_chunk::{PalettedContainer, SectionKind, VoxelId};
use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::{Deserialize, Serialize, Serializer};

use crate::ErrorKind;
use crate::palette::{BlockStateList, PaletteLookup};
use crate::retrogen::RetroGen;
use crate::status::ChunkStatus;

pub const LIGHT_BYTES: usize = 2048;

/// One nibble per cell, indexed the same way block states are.
pub type Light = mcrs_minecraft_chunk::SectionNibbles;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub y: i8,
    pub block_states: Option<PalettedContainer<VoxelId, { Blocks::SIZE }>>,
    pub biomes: Option<PalettedContainer<u8, { Biomes::SIZE }>>,
    pub noise_biomes: Option<PalettedContainer<u8, { NoiseBiomes::SIZE }>>,
    pub block_light: Option<Light>,
    pub sky_light: Option<Light>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub pos: ColumnPos,
    /// `yPos`: the section Y the saved section array starts at.
    pub min_section_y: i32,
    pub status: ChunkStatus,
    pub is_light_on: bool,
    pub inhabited_time: i64,
    pub last_update: i64,
    pub heightmaps: BTreeMap<String, Vec<i64>>,
    pub block_entities: Vec<NbtCompound>,
    pub retrogen: Option<RetroGen>,
    pub sections: Vec<Section>,
}

impl Chunk {
    /// Answers the state the cell held. A changed cell leaves the saved light and
    /// heightmaps describing the old blocks, so both are dropped for the loader to
    /// recompute.
    pub fn set_block(&mut self, pos: BlockPos, state: VoxelId) -> Result<VoxelId, ErrorKind> {
        let (x, y, z) = pos.into();
        if ColumnPos::from(pos) != self.pos {
            return Err(ErrorKind::BlockOutsideChunk {
                x,
                y,
                z,
                chunk_x: self.pos.x,
                chunk_z: self.pos.z,
            });
        }
        let section_y = y >> 4;
        let states = self
            .sections
            .iter_mut()
            .find(|section| i32::from(section.y) == section_y)
            .and_then(|section| section.block_states.as_mut())
            .ok_or(ErrorKind::NoBlockStates {
                x: self.pos.x,
                z: self.pos.z,
                section_y,
            })?;
        let previous = states.set(
            (x & 15) as usize,
            (y & 15) as usize,
            (z & 15) as usize,
            state,
        );
        if previous != state {
            self.is_light_on = false;
            self.heightmaps.clear();
            for section in &mut self.sections {
                section.block_light = None;
                section.sky_light = None;
            }
        }
        Ok(previous)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawPalettedContainer {
    pub(crate) palette: BlockStateList,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<PackedData>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RawSection {
    #[serde(rename = "Y")]
    pub(crate) y: i8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) block_states: Option<RawPalettedContainer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) biomes: Option<RawPalettedContainer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) noise_biomes: Option<RawPalettedContainer>,
    #[serde(rename = "BlockLight", skip_serializing_if = "Option::is_none")]
    pub(crate) block_light: Option<RawLight>,
    #[serde(rename = "SkyLight", skip_serializing_if = "Option::is_none")]
    pub(crate) sky_light: Option<RawLight>,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct RawChunk {
    #[serde(rename = "DataVersion")]
    pub(crate) data_version: i32,
    #[serde(rename = "xPos")]
    pub(crate) x_pos: i32,
    #[serde(rename = "zPos")]
    pub(crate) z_pos: i32,
    #[serde(rename = "yPos")]
    pub(crate) y_pos: i32,
    pub(crate) status: ChunkStatus,
    #[serde(default)]
    pub(crate) sections: Vec<RawSection>,
    #[serde(
        rename = "Heightmaps",
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "long_array_map"
    )]
    pub(crate) heightmaps: BTreeMap<String, Vec<i64>>,
    /// Vanilla writes the flag only when set.
    #[serde(rename = "isLightOn", default, skip_serializing_if = "is_false")]
    pub(crate) is_light_on: bool,
    #[serde(default)]
    pub(crate) block_entities: Vec<NbtCompound>,
    #[serde(rename = "InhabitedTime", default)]
    pub(crate) inhabited_time: i64,
    #[serde(rename = "LastUpdate", default)]
    pub(crate) last_update: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) retrogen: Option<RetroGen>,
}

fn is_false(flag: &bool) -> bool {
    !*flag
}

fn long_array_map<S: Serializer>(
    map: &BTreeMap<String, Vec<i64>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    struct LongArray<'a>(&'a [i64]);

    impl Serialize for LongArray<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            mcrs_minecraft_nbt::nbt_long_array(self.0, serializer)
        }
    }

    serializer.collect_map(map.iter().map(|(name, data)| (name, LongArray(data))))
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
            expected: VERSION.world_version,
        }),
        Some(found) if found != VERSION.world_version => Some(ErrorKind::DataVersion {
            found,
            expected: VERSION.world_version,
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
    if raw.data_version != VERSION.world_version {
        return Err(ErrorKind::DataVersion {
            found: raw.data_version,
            expected: VERSION.world_version,
        });
    }
    let keeps_noise_biomes = raw.status != ChunkStatus::Full;
    Ok(Chunk {
        pos: ColumnPos::new(raw.x_pos, raw.z_pos),
        min_section_y: raw.y_pos,
        status: raw.status,
        is_light_on: raw.is_light_on,
        inhabited_time: raw.inhabited_time,
        last_update: raw.last_update,
        heightmaps: raw.heightmaps,
        block_entities: raw.block_entities,
        retrogen: raw.retrogen,
        sections: raw
            .sections
            .into_iter()
            .map(|section| parse_section(section, keeps_noise_biomes, blocks, biomes))
            .collect::<Result<_, _>>()?,
    })
}

fn parse_section(
    raw: RawSection,
    keeps_noise_biomes: bool,
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
        noise_biomes: raw
            .noise_biomes
            .map(|c| unpack::<NoiseBiomes, _, _>(c, biomes, y, "noise_biomes"))
            .transpose()?
            .filter(|_| keeps_noise_biomes),
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
pub(crate) struct PackedData(pub(crate) Box<[i64]>);

impl Serialize for PackedData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        mcrs_minecraft_nbt::nbt_long_array(&*self.0, serializer)
    }
}

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
pub(crate) struct RawLight(pub(crate) Vec<u8>);

impl Serialize for RawLight {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        mcrs_minecraft_nbt::nbt_byte_array(&self.0, serializer)
    }
}

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
