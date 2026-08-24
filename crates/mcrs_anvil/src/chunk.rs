use std::collections::BTreeMap;
use std::io::Cursor;
use std::marker::PhantomData;

use mcrs_nbt::compound::NbtCompound;
use mcrs_palette::SectionKind;
use serde::Deserialize;

use crate::palette::{BlockStateLookup, Palette, Properties};
use crate::{DATA_VERSION, ErrorKind};

pub const LIGHT_BYTES: usize = 2048;

/// Palette entries plus one index per cell, in `Strategy.getIndex` order. Cells
/// stay packed: the consumer walks them once anyway, through `remap_into`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalettedContainer<K> {
    pub palette: Palette,
    cells: Cells,
    kind: PhantomData<K>,
}

pub type BlockStates = PalettedContainer<mcrs_palette::Blocks>;
pub type Biomes = PalettedContainer<mcrs_palette::Biomes>;

impl<K: SectionKind> PalettedContainer<K> {
    pub const ENTRY_COUNT: usize = K::ENTRY_COUNT;

    pub fn index(x: usize, y: usize, z: usize) -> usize {
        K::index(x, y, z)
    }

    /// Which palette entry the cell holds.
    pub fn palette_index(&self, x: usize, y: usize, z: usize) -> usize {
        match &self.cells {
            Cells::Uniform => 0,
            Cells::Packed { bits, data } => {
                mcrs_palette::entry_at(*bits, data, Self::index(x, y, z)) as usize
            }
        }
    }

    pub fn name(&self, x: usize, y: usize, z: usize) -> &str {
        self.palette.name(self.palette_index(x, y, z))
    }

    pub fn properties(&self, x: usize, y: usize, z: usize) -> Properties<'_> {
        self.palette.properties(self.palette_index(x, y, z))
    }

    /// One id per palette entry, resolved while the names are still borrowed.
    pub fn resolve_palette<R: BlockStateLookup>(
        &self,
        registry: &R,
    ) -> Result<Vec<u32>, ErrorKind> {
        (0..self.palette.len())
            .map(|i| {
                let name = self.palette.name(i);
                registry
                    .resolve(name, self.palette.properties(i))
                    .ok_or_else(|| ErrorKind::UnknownPaletteEntry {
                        name: name.to_string(),
                    })
            })
            .collect()
    }

    pub fn unpack_into(&self, out: &mut [u16]) {
        assert_eq!(out.len(), Self::ENTRY_COUNT);
        match &self.cells {
            Cells::Uniform => out.fill(0),
            Cells::Packed { bits, data } => mcrs_palette::unpack_into(*bits, data, out)
                .expect("the data length was checked at load"),
        }
    }

    pub fn remap_into<T: Copy>(&self, entries: &[T], out: &mut [T]) {
        assert_eq!(out.len(), Self::ENTRY_COUNT);
        assert_eq!(entries.len(), self.palette.len());
        match &self.cells {
            Cells::Uniform => out.fill(entries[0]),
            Cells::Packed { bits, data } => mcrs_palette::remap_into(*bits, data, entries, out)
                .expect("the data length was checked at load"),
        }
    }

    fn unpack(raw: RawPalettedContainer, y: i8, field: &'static str) -> Result<Self, ErrorKind> {
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
            return Ok(Self {
                palette: raw.palette,
                cells: Cells::Uniform,
                kind: PhantomData,
            });
        }

        let Some(data) = raw.data else {
            return Err(ErrorKind::MissingData { y, field, bits });
        };
        let data = data.0;
        mcrs_palette::check_len(bits, &data, Self::ENTRY_COUNT).map_err(|e| {
            ErrorKind::DataLength {
                y,
                field,
                found: e.found,
                expected: e.expected,
                bits,
            }
        })?;

        if mcrs_palette::any_entry_past(bits, &data, Self::ENTRY_COUNT, len) {
            let index = mcrs_palette::first_entry_past(bits, &data, Self::ENTRY_COUNT, len)
                .expect("the maximum is already past the palette");
            return Err(ErrorKind::PaletteIndex {
                y,
                field,
                index,
                len,
            });
        }

        Ok(Self {
            palette: raw.palette,
            cells: Cells::Packed { bits, data },
            kind: PhantomData,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cells {
    Uniform,
    Packed { bits: u32, data: Box<[i64]> },
}

/// One nibble per cell, indexed the same way block states are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Light(pub Box<[u8; LIGHT_BYTES]>);

impl Light {
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        let index = BlockStates::index(x, y, z);
        self.0[index >> 1] >> (4 * (index & 1)) & 0xf
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub y: i8,
    pub block_states: Option<BlockStates>,
    pub biomes: Option<Biomes>,
    pub block_light: Option<Light>,
    pub sky_light: Option<Light>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
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
    palette: Palette,
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
    let raw: RawChunkVersion = mcrs_nbt::from_bytes(Cursor::new(nbt)).ok()?;
    match raw.data_version {
        None => Some(ErrorKind::MissingDataVersion {
            expected: DATA_VERSION,
        }),
        Some(found) if found != DATA_VERSION => Some(ErrorKind::DataVersion {
            found,
            expected: DATA_VERSION,
        }),
        Some(_) => None,
    }
}

pub fn parse(nbt: &[u8]) -> Result<Chunk, ErrorKind> {
    let raw: RawChunk = match mcrs_nbt::from_bytes(Cursor::new(nbt)) {
        Ok(raw) => raw,
        Err(err) => return Err(wrong_version(nbt).unwrap_or_else(|| err.into())),
    };
    if raw.data_version != DATA_VERSION {
        return Err(ErrorKind::DataVersion {
            found: raw.data_version,
            expected: DATA_VERSION,
        });
    }
    Ok(Chunk {
        x: raw.x_pos,
        z: raw.z_pos,
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
            .map(parse_section)
            .collect::<Result<_, _>>()?,
    })
}

fn parse_section(raw: RawSection) -> Result<Section, ErrorKind> {
    let y = raw.y;
    Ok(Section {
        y,
        block_states: raw
            .block_states
            .map(|c| BlockStates::unpack(c, y, "block_states"))
            .transpose()?,
        biomes: raw
            .biomes
            .map(|c| Biomes::unpack(c, y, "biomes"))
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
    Ok(Light(bytes))
}
