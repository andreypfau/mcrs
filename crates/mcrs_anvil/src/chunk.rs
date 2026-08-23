use std::collections::BTreeMap;
use std::io::Cursor;

use mcrs_nbt::compound::NbtCompound;
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::{DATA_VERSION, ErrorKind};

pub const LIGHT_BYTES: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockState {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Properties", default)]
    pub properties: BTreeMap<String, String>,
}

/// Palette entries plus one index per cell, in `Strategy.getIndex` order:
/// `(y << AXIS_BITS | z) << AXIS_BITS | x`. `MIN_BITS` is the narrowest width the
/// strategy will store a non-empty palette at, which is why a three-entry block
/// palette is packed at four bits rather than two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalettedContainer<T, const AXIS_BITS: u32, const MIN_BITS: u32> {
    pub palette: Vec<T>,
    entries: Box<[u16]>,
}

pub type BlockStates = PalettedContainer<BlockState, 4, 4>;
pub type Biomes = PalettedContainer<String, 2, 1>;

impl<T, const AXIS_BITS: u32, const MIN_BITS: u32> PalettedContainer<T, AXIS_BITS, MIN_BITS> {
    pub const ENTRY_COUNT: usize = 1 << (3 * AXIS_BITS);

    pub const fn index(x: usize, y: usize, z: usize) -> usize {
        (y << AXIS_BITS | z) << AXIS_BITS | x
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> &T {
        &self.palette[self.entries[Self::index(x, y, z)] as usize]
    }

    pub fn entries(&self) -> &[u16] {
        &self.entries
    }

    fn unpack(raw: RawPalettedContainer<T>, y: i8, field: &'static str) -> Result<Self, ErrorKind> {
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
        let bits = match bits_for_distinct_values(len) {
            0 => {
                if raw.data.is_some() {
                    return Err(ErrorKind::UnexpectedData { y, field });
                }
                return Ok(Self {
                    palette: raw.palette,
                    entries: vec![0u16; Self::ENTRY_COUNT].into_boxed_slice(),
                });
            }
            bits => bits.max(MIN_BITS),
        };

        let Some(data) = raw.data else {
            return Err(ErrorKind::MissingData { y, field, bits });
        };
        let per_long = 64 / bits as usize;
        let expected = Self::ENTRY_COUNT.div_ceil(per_long);
        if data.len() != expected {
            return Err(ErrorKind::DataLength {
                y,
                field,
                found: data.len(),
                expected,
                bits,
            });
        }

        let mask = (1u64 << bits) - 1;
        let mut entries = vec![0u16; Self::ENTRY_COUNT];
        for (cells, &word) in entries.chunks_mut(per_long).zip(data.iter()) {
            let mut word = word as u64;
            for cell in cells {
                *cell = (word & mask) as u16;
                word >>= bits;
            }
        }

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
            entries: entries.into_boxed_slice(),
        })
    }
}

/// `Mth.ceillog2`: the palette size a stored entry must be able to address.
fn bits_for_distinct_values(count: usize) -> u32 {
    match count {
        0 | 1 => 0,
        n => usize::BITS - (n - 1).leading_zeros(),
    }
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
struct RawPalettedContainer<T> {
    palette: Vec<T>,
    data: Option<Vec<i64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSection {
    #[serde(rename = "Y")]
    y: i8,
    block_states: Option<RawPalettedContainer<BlockState>>,
    biomes: Option<RawPalettedContainer<String>>,
    #[serde(rename = "BlockLight")]
    block_light: Option<Vec<u8>>,
    #[serde(rename = "SkyLight")]
    sky_light: Option<Vec<u8>>,
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
            .map(|bytes| light(bytes, y, "BlockLight"))
            .transpose()?,
        sky_light: raw
            .sky_light
            .map(|bytes| light(bytes, y, "SkyLight"))
            .transpose()?,
    })
}

fn light(bytes: Vec<u8>, y: i8, field: &'static str) -> Result<Light, ErrorKind> {
    let found = bytes.len();
    let bytes: Box<[u8; LIGHT_BYTES]> = bytes
        .into_boxed_slice()
        .try_into()
        .map_err(|_| ErrorKind::LightLength { y, field, found })?;
    Ok(Light(bytes))
}
