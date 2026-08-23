use std::collections::BTreeMap;
use std::io::Cursor;
use std::marker::PhantomData;

use mcrs_nbt::compound::NbtCompound;
use mcrs_palette::SectionKind;
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::palette::{BlockStateLookup, Palette, Properties};
use crate::{DATA_VERSION, ErrorKind};

pub const LIGHT_BYTES: usize = 2048;

/// Palette entries plus one index per cell, in `Strategy.getIndex` order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalettedContainer<K> {
    pub palette: Palette,
    entries: Cells,
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
        match &self.entries {
            Cells::Uniform => 0,
            Cells::Packed(cells) => cells[Self::index(x, y, z)] as usize,
        }
    }

    pub fn name(&self, x: usize, y: usize, z: usize) -> &str {
        self.palette.name(self.palette_index(x, y, z))
    }

    pub fn properties(&self, x: usize, y: usize, z: usize) -> Properties<'_> {
        self.palette.properties(self.palette_index(x, y, z))
    }

    /// One id per palette entry, resolved while the names are still borrowed.
    /// Combine with `entries()` through `mcrs_palette::remap_into` to get a
    /// cell-indexed array without a second pass over the names.
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

    pub fn entries(&self) -> &[u16] {
        match &self.entries {
            Cells::Uniform => &ZEROS[..Self::ENTRY_COUNT],
            Cells::Packed(cells) => cells,
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
                entries: Cells::Uniform,
                kind: PhantomData,
            });
        }

        let Some(data) = raw.data else {
            return Err(ErrorKind::MissingData { y, field, bits });
        };
        let mut entries = vec![0u16; Self::ENTRY_COUNT];
        mcrs_palette::unpack_into(bits, &data, &mut entries).map_err(|e| {
            ErrorKind::DataLength {
                y,
                field,
                found: e.found,
                expected: e.expected,
                bits,
            }
        })?;

        // One pass over a u16 slice vectorizes; a per-cell bound check does not.
        if entries
            .iter()
            .copied()
            .max()
            .is_some_and(|m| m as usize >= len)
        {
            let index = entries
                .iter()
                .copied()
                .find(|&e| e as usize >= len)
                .unwrap();
            return Err(ErrorKind::PaletteIndex {
                y,
                field,
                index,
                len,
            });
        }

        Ok(Self {
            palette: raw.palette,
            entries: Cells::Packed(entries.into_boxed_slice()),
            kind: PhantomData,
        })
    }
}

/// A single-entry palette addresses no cells, so it stores none. `entries()`
/// still answers with a slice, which is why the zeroes are static rather than
/// allocated per container.
static ZEROS: [u16; 4096] = [0; 4096];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cells {
    Uniform,
    Packed(Box<[u16]>),
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
    data: Option<Vec<i64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

/// Underscored fields exist so that `deny_unknown_fields` accepts a vanilla chunk
/// while still rejecting a key this decoder has never heard of. Their contents
/// have no consumer yet; give one a type when it grows one.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(rename = "block_ticks")]
    _block_ticks: Option<IgnoredAny>,
    #[serde(rename = "fluid_ticks")]
    _fluid_ticks: Option<IgnoredAny>,
    #[serde(rename = "PostProcessing")]
    _post_processing: Option<IgnoredAny>,
    #[serde(rename = "structures")]
    _structures: Option<IgnoredAny>,
    #[serde(rename = "entities")]
    _entities: Option<IgnoredAny>,
    #[serde(rename = "UpgradeData")]
    _upgrade_data: Option<IgnoredAny>,
    #[serde(rename = "blending_data")]
    _blending_data: Option<IgnoredAny>,
    #[serde(rename = "below_zero_retrogen")]
    _below_zero_retrogen: Option<IgnoredAny>,
}

pub fn parse(nbt: &[u8]) -> Result<Chunk, ErrorKind> {
    let raw: RawChunk = mcrs_nbt::from_bytes(Cursor::new(nbt))?;
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
